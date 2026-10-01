'use strict';

/**
 * 色温算法（黑体辐射近似，Tanner Helland 常数，6500K 归一）
 * 设计文档 §5.1：docs/superpowers/specs/2026-10-01--design.md:152
 *
 * 纯函数模块：无 I/O、无副作用，可独立单测。
 *
 * 驱动安全钳制（2026-10-01 H2 反证实测）：
 *   本机驱动拒绝任何「通道最大值 < 32768」的 ramp（Intel 类 50% 规则，
 *   细扫探针确认边界精确在 32768，R/G/B 对称生效）。
 *   buildSafeLut 在生成 LUT 时对被压过低的通道自动提升总增益，
 *   保证所有通道最大值 ≥ SAFE_MAX_VALUE，并回报实际生效色温。
 */

function clamp(v, lo, hi) {
  return Math.min(hi, Math.max(lo, v));
}

/** 本机实测的驱动接受边界：每通道最大值必须 ≥ 该值（否则 SetDeviceGammaRamp 整体拒绝） */
const SAFE_MAX_VALUE = 32768;

/**
 * 黑体辐射近似：色温 K → RGB 分量（0-255 尺度）
 * @param {number} K 绝对色温（1000-40000，本项目用 2000-10000）
 */
function rgbAtKelvin(K) {
  const t = K / 100;
  let r;
  let g;
  let b;

  if (t <= 66) {
    r = 255;
    g = 99.4708025861 * Math.log(t) - 161.1195681661;
  } else {
    r = 329.698727446 * Math.pow(t - 60, -0.1332047592);
    g = 288.1221695283 * Math.pow(t - 60, -0.0755148492);
  }

  if (t >= 66) {
    b = 255;
  } else if (t <= 19) {
    b = 0;
  } else {
    b = 138.5177312231 * Math.log(t - 10) - 305.0447927307;
  }

  return { r: clamp(r, 0, 255), g: clamp(g, 0, 255), b: clamp(b, 0, 255) };
}

// 参考白点：6500K（sRGB D65 近似）
const REF_6500 = rgbAtKelvin(6500);

/**
 * 色温增益：relative to 6500K，恒在 [0,1]（只衰减，不增亮）
 * gain(6500) === (1,1,1) 精确成立
 */
function temperatureGain(K) {
  const cur = rgbAtKelvin(K);
  return {
    r: clamp(cur.r / REF_6500.r, 0, 1),
    g: clamp(cur.g / REF_6500.g, 0, 1),
    b: clamp(cur.b / REF_6500.b, 0, 1)
  };
}

/**
 * 生成 3×256 gamma LUT（Uint16Array，[R0..R255, G0..G255, B0..B255]）
 * final[c][i] = clamp(round(orig[c][i] * gain_temp[c] * gain_brightness), 0, 65535)
 *
 * 基底是原始 ramp（启动时存档）——保留显示器硬件校准。
 * 恒等式：buildLut(orig, 6500, 100) 与 orig 逐元素一致。
 */
function buildLut(orig, temperature, brightness) {
  const gain = temperatureGain(temperature);
  const brightnessFactor = brightness / 100;
  const gains = [gain.r * brightnessFactor, gain.g * brightnessFactor, gain.b * brightnessFactor];

  const out = new Uint16Array(768);
  for (let c = 0; c < 3; c++) {
    const g = gains[c];
    const base = c * 256;
    for (let i = 0; i < 256; i++) {
      out[base + i] = clamp(Math.round(orig[base + i] * g), 0, 65535);
    }
  }
  return out;
}

/** 通道最大值（原始 ramp 通常单调，但按实际扫描） */
function channelMax(orig, channel) {
  let m = 0;
  const base = channel * 256;
  for (let i = 0; i < 256; i++) {
    if (orig[base + i] > m) m = orig[base + i];
  }
  return m;
}

/**
 * 反解：给定蓝通道增益，求等效色温 K（gain_b 关于 K 单调）
 * @returns {number} 100K 取整的等效色温
 */
function kelvinForGainB(targetGain) {
  if (targetGain >= 1) return 6500;
  if (targetGain <= 0) return 2000;
  let lo = 2000;
  let hi = 6500;
  for (let i = 0; i < 50; i++) {
    const mid = (lo + hi) / 2;
    if (temperatureGain(mid).b < targetGain) lo = mid;
    else hi = mid;
  }
  return Math.round((lo + hi) / 2 / 100) * 100;
}

/** 冷区上界：10000K 处的红增益（≈0.791），作为 kelvinForGainR 的判据下界 */
const COLD_R_GAIN_10000 = temperatureGain(10000).r;

/**
 * 反解：给定红通道增益，求等效色温 K（冷区，gain_r 在 [6500,10000] 上单调不增）
 * @param {number} targetGain 相对 6500K 的红增益
 * @returns {number} 100K 取整的等效色温
 */
function kelvinForGainR(targetGain) {
  if (targetGain >= 1) return 6500;
  if (targetGain <= COLD_R_GAIN_10000) return 10000;
  let lo = 6500;
  let hi = 10000;
  for (let i = 0; i < 50; i++) {
    const mid = (lo + hi) / 2;
    if (temperatureGain(mid).r > targetGain) lo = mid;
    else hi = mid;
  }
  return Math.round((lo + hi) / 2 / 100) * 100;
}

/**
 * 统一反解：由实际生效增益（effGain）与亮度因子求等效色温 K。
 * 蓝增益先被压低 → 暖区（kelvinForGainB）；
 * 否则红增益被压低 → 冷区（kelvinForGainR）；
 * 否则为中性 6500K。
 * @param {{r:number,g:number,b:number}} effGain 实际生效增益（相对 6500K）
 * @param {number} brightnessFactor 亮度因子（brightness/100）
 * @returns {number} 100K 取整的等效色温
 */
function equivalentKelvin(effGain, brightnessFactor) {
  const bf = brightnessFactor || 1;
  const kb = Math.min(1, effGain.b / bf);
  const kr = Math.min(1, effGain.r / bf);
  if (kb < 1) return kelvinForGainB(kb);
  if (kr < 1) return kelvinForGainR(kr);
  return 6500;
}

/**
 * 安全 LUT：与 buildLut 相同的映射，但保证每通道最大值 ≥ safeMax。
 * 被压过低的通道自动提升总增益——以「牺牲部分暖度/暗度」换取驱动接受，
 * 通过 effectiveTemperature 回报实际生效色温。
 *
 * @param {Uint16Array} orig 原始 ramp（768 项）
 * @param {number} temperature 请求色温（2000-10000）
 * @param {number} brightness 请求亮度（50-100）
 * @param {number} [safeMax] 安全边界（默认本机实测值 32768）
 */
function buildSafeLut(orig, temperature, brightness, safeMax = SAFE_MAX_VALUE) {
  const gain = temperatureGain(temperature);
  const brightnessFactor = brightness / 100;
  const effGain = {
    r: gain.r * brightnessFactor,
    g: gain.g * brightnessFactor,
    b: gain.b * brightnessFactor
  };
  let clamped = false;

  const channels = ['r', 'g', 'b'];
  for (let c = 0; c < 3; c++) {
    const name = channels[c];
    const maxOrig = channelMax(orig, c);
    if (maxOrig === 0) continue;
    const minTotalGain = safeMax / maxOrig;
    if (effGain[name] < minTotalGain) {
      effGain[name] = Math.min(1, minTotalGain);
      clamped = true;
    }
  }

  const lut = new Uint16Array(768);
  for (let c = 0; c < 3; c++) {
    const g = effGain[channels[c]];
    const base = c * 256;
    for (let i = 0; i < 256; i++) {
      lut[base + i] = clamp(Math.round(orig[base + i] * g), 0, 65535);
    }
  }

  const effectiveTemperature = equivalentKelvin(effGain, brightnessFactor || 1);

  return { lut, clamped, effectiveTemperature, effectiveGains: effGain };
}

module.exports = {
  rgbAtKelvin,
  temperatureGain,
  buildLut,
  buildSafeLut,
  kelvinForGainB,
  kelvinForGainR,
  equivalentKelvin,
  SAFE_MAX_VALUE
};
