'use strict';

/**
 * FFI 集成自检（真实 GPU 写入，运行中屏幕会短暂变色后复原）
 *
 * 流程：读原始 ramp → 4500K 安全写入+读回校验 → 2700K+50%（H2 越界组合，
 *       应被安全钳制后写入成功）→ 6500K 恒等恢复校验 → 冷区 8000K / 10000K
 *       写入+读回校验（蓝通道为最大值、红通道低于原始、effectiveTemperature 偏差 ≤300K）
 *       → 恢复原始 ramp（try/finally 保证）
 *
 * 背景（2026-10-01 实测）：本机驱动要求每通道最大值 ≥ 32768，
 * 钳制逻辑见 temperature.buildSafeLut。
 *
 * 判据：全部用例通过 exit 0；任一失败 exit 1。屏幕在任何路径下都会被恢复。
 */
const gamma = require('../src/main/gamma');
const { buildSafeLut } = require('../src/main/temperature');

let allOk = true;
let original = null;

function check(name, cond, detail) {
  const tag = cond ? 'PASS' : 'FAIL';
  console.log(`[selftest] ${tag} ${name}${detail ? ' — ' + detail : ''}`);
  if (!cond) allOk = false;
  return cond;
}

/** 通道最大值（channel：0=R,1=G,2=B），arr 为 768 项 LUT/ramp */
function channelMax(arr, channel) {
  const base = channel * 256;
  let m = 0;
  for (let i = 0; i < 256; i++) {
    if (arr[base + i] > m) m = arr[base + i];
  }
  return m;
}

/** 两 LUT 指定通道逐元素最大绝对偏差 */
function channelDiff(a, b, channel) {
  const base = channel * 256;
  let d = 0;
  for (let i = 0; i < 256; i++) {
    const diff = Math.abs(a[base + i] - b[base + i]);
    if (diff > d) d = diff;
  }
  return d;
}

try {
  original = gamma.readRamp();
  check('读取原始 ramp', true, `R/G/B[255]=${original[255]}/${original[256 + 255]}/${original[512 + 255]}`);

  // 用例 1：4500K + 100%（办公预设，安全组合，不应触发钳制）
  const r1 = buildSafeLut(original, 4500, 100);
  const ok1 = gamma.writeRamp(r1.lut);
  const rb1 = gamma.readRamp();
  check(
    '写入 4500K 且读回一致（无钳制）',
    ok1 && !r1.clamped && Math.abs(rb1[512 + 255] - r1.lut[512 + 255]) <= 1,
    `clamped=${r1.clamped} 写=${r1.lut[512 + 255]} 读回=${rb1[512 + 255]}`
  );

  // 用例 2：2700K + 50%（H2 越界组合 → 安全钳制应为可写入）
  const r2 = buildSafeLut(original, 2700, 50);
  const ok2 = gamma.writeRamp(r2.lut);
  const rb2 = gamma.readRamp();
  check(
    '写入 2700K+50%（钳制后）且读回一致',
    ok2 && Math.abs(rb2[512 + 255] - r2.lut[512 + 255]) <= 1,
    `clamped=${r2.clamped} 生效K≈${r2.effectiveTemperature} 写=${r2.lut[512 + 255]} 读回=${rb2[512 + 255]}`
  );

  // 用例 3：6500K + 100%（恒等路径）
  const r3 = buildSafeLut(original, 6500, 100);
  gamma.writeRamp(r3.lut);
  const rb3 = gamma.readRamp();
  const identity = Array.from(rb3).every((v, i) => v === original[i]);
  check('恢复原色（恒等 LUT）逐元素一致', identity);

  // 用例 4：8000K + 100%（冷区，蓝增益 1.0，红增益被压低）
  const r4 = buildSafeLut(original, 8000, 100);
  const ok4 = gamma.writeRamp(r4.lut);
  const rb4 = gamma.readRamp();
  const diffB4 = channelDiff(rb4, r4.lut, 2);
  check(
    '写入 8000K 且读回蓝通道一致（≤1）',
    ok4 && diffB4 <= 1,
    `clamped=${r4.clamped} 蓝通道最大偏差=${diffB4}`
  );
  check(
    '8000K 蓝通道为三通道最大（冷色方向）',
    channelMax(r4.lut, 2) >= channelMax(r4.lut, 1) && channelMax(r4.lut, 2) >= channelMax(r4.lut, 0),
    `R/G/B max=${channelMax(r4.lut, 0)}/${channelMax(r4.lut, 1)}/${channelMax(r4.lut, 2)}`
  );
  check(
    '8000K 红通道低于原始',
    channelMax(r4.lut, 0) < channelMax(original, 0),
    `LUT红max=${channelMax(r4.lut, 0)} 原始红max=${channelMax(original, 0)}`
  );
  check(
    '8000K effectiveTemperature 偏差 ≤300K',
    Math.abs(r4.effectiveTemperature - 8000) <= 300,
    `生效K=${r4.effectiveTemperature}`
  );

  // 用例 5：10000K + 100%（冷区上界）
  const r5 = buildSafeLut(original, 10000, 100);
  const ok5 = gamma.writeRamp(r5.lut);
  const rb5 = gamma.readRamp();
  const diffB5 = channelDiff(rb5, r5.lut, 2);
  check(
    '写入 10000K 且读回蓝通道一致（≤1）',
    ok5 && diffB5 <= 1,
    `clamped=${r5.clamped} 蓝通道最大偏差=${diffB5}`
  );
  check(
    '10000K 蓝通道为三通道最大（冷色方向）',
    channelMax(r5.lut, 2) >= channelMax(r5.lut, 1) && channelMax(r5.lut, 2) >= channelMax(r5.lut, 0),
    `R/G/B max=${channelMax(r5.lut, 0)}/${channelMax(r5.lut, 1)}/${channelMax(r5.lut, 2)}`
  );
  check(
    '10000K 红通道低于原始',
    channelMax(r5.lut, 0) < channelMax(original, 0),
    `LUT红max=${channelMax(r5.lut, 0)} 原始红max=${channelMax(original, 0)}`
  );
  check(
    '10000K effectiveTemperature 偏差 ≤300K',
    Math.abs(r5.effectiveTemperature - 10000) <= 300,
    `生效K=${r5.effectiveTemperature}`
  );
} catch (err) {
  allOk = false;
  console.error('[selftest] ERROR:', err.message);
} finally {
  if (original) {
    try {
      const okRestore = gamma.writeRamp(original);
      console.log(`[selftest] 恢复原始 ramp: ${okRestore ? 'OK' : 'FAILED'}`);
      if (!okRestore) allOk = false;
    } catch (err) {
      allOk = false;
      console.error('[selftest] 恢复失败:', err.message);
    }
  }
}

console.log('[selftest]', allOk ? 'ALL PASS' : 'SOME FAILED');
process.exit(allOk ? 0 : 1);
