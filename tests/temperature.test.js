'use strict';

// W2-T2.1（RED）：色温算法与 LUT 生成的行为锁定
// 期望值独立推导：黑体近似公式（Tanner Helland 常数），6500K 归一
const { test } = require('node:test');
const assert = require('node:assert/strict');

const { rgbAtKelvin, temperatureGain, buildLut, buildSafeLut } = require('../src/main/temperature');

function approx(actual, expected, tol, msg) {
  assert.ok(
    Math.abs(actual - expected) <= tol,
    `${msg || ''} expected ${expected} ±${tol}, got ${actual}`
  );
}

test('rgbAtKelvin: 6500K 参考白点（独立推导：g≈254.11, b≈250.04）', () => {
  const rgb = rgbAtKelvin(6500);
  approx(rgb.r, 255, 0.001, 'r');
  approx(rgb.g, 254.11, 0.05, 'g');
  approx(rgb.b, 250.04, 0.05, 'b');
});

test('temperatureGain: 6500K 精确等于 (1,1,1)', () => {
  const gain = temperatureGain(6500);
  assert.equal(gain.r, 1);
  assert.equal(gain.g, 1);
  assert.equal(gain.b, 1);
});

test('temperatureGain: 2700K 暖色（独立推导：g≈0.656, b≈0.350）', () => {
  const gain = temperatureGain(2700);
  approx(gain.r, 1.0, 0.001, 'r');
  approx(gain.g, 0.656, 0.005, 'g');
  approx(gain.b, 0.350, 0.005, 'b');
});

test('temperatureGain: 4500K 中等暖（独立推导：g≈0.856, b≈0.750）', () => {
  const gain = temperatureGain(4500);
  approx(gain.r, 1.0, 0.001, 'r');
  approx(gain.g, 0.856, 0.005, 'g');
  approx(gain.b, 0.750, 0.005, 'b');
});

test('temperatureGain: 增益恒在 [0,1]——只衰减不增亮', () => {
  for (let k = 2000; k <= 6500; k += 100) {
    const gain = temperatureGain(k);
    for (const c of ['r', 'g', 'b']) {
      assert.ok(gain[c] >= 0 && gain[c] <= 1, `K=${k} ${c}=${gain[c]}`);
    }
  }
});

test('temperatureGain: 蓝增益随 K 降低单调不增', () => {
  let prev = Infinity;
  for (let k = 6500; k >= 2000; k -= 100) {
    const b = temperatureGain(k).b;
    assert.ok(b <= prev + 1e-12, `K=${k} b=${b} prev=${prev}`);
    prev = b;
  }
});

test('buildLut: 6500K + 100% 亮度 => 与原始 ramp 逐元素一致（恒等）', () => {
  const orig = new Uint16Array(768);
  for (let i = 0; i < 768; i++) orig[i] = (i % 256) * 257; // 模拟标准 16 位 ramp
  const lut = buildLut(orig, 6500, 100);
  assert.deepEqual(Array.from(lut), Array.from(orig));
});

test('buildLut: 输出恒被 clamp 在 [0,65535]', () => {
  const hi = buildLut(new Uint16Array(768).fill(65535), 2000, 50);
  for (let i = 0; i < hi.length; i++) {
    assert.ok(hi[i] >= 0 && hi[i] <= 65535, `idx=${i} val=${hi[i]}`);
  }
  const lo = buildLut(new Uint16Array(768).fill(0), 6500, 50);
  for (let i = 0; i < lo.length; i++) assert.equal(lo[i], 0);
});

test('buildLut: 6500K + 50% 亮度 => 各通道≈原始值一半', () => {
  const orig = new Uint16Array(768).fill(65535);
  const lut = buildLut(orig, 6500, 50);
  approx(lut[255], 32768, 1, 'R[255]');
  approx(lut[512 + 255], 32768, 1, 'B[255]');
});

test('buildLut: 2700K 时蓝通道显著低于红通道（暖色方向）', () => {
  const orig = new Uint16Array(768).fill(65535);
  const lut = buildLut(orig, 2700, 100);
  const r = lut[255];
  const b = lut[512 + 255];
  assert.ok(b < r * 0.4, `B=${b} 应显著低于 R=${r}`);
});

// ---- 驱动安全钳制（H2 反证实测：本机每通道最大值必须 ≥ 32768，否则整体拒绝）----

test('buildSafeLut: 6500K+100%（原色）无需钳制，与原始一致', () => {
  const orig = new Uint16Array(768);
  for (let i = 0; i < 768; i++) orig[i] = (i % 256) * 257;
  const { lut, clamped } = buildSafeLut(orig, 6500, 100);
  assert.deepEqual(Array.from(lut), Array.from(orig));
  assert.equal(clamped, false);
});

test('buildSafeLut: 4500K+100%（安全组合）不触发钳制', () => {
  const orig = new Uint16Array(768);
  for (let i = 0; i < 768; i++) orig[i] = (i % 256) * 257;
  const { clamped } = buildSafeLut(orig, 4500, 100);
  assert.equal(clamped, false);
});

test('buildSafeLut: 2700K+100%（越界组合）自动钳制——蓝通道最大值 ≥ 32768，生效色温被抬高', () => {
  const orig = new Uint16Array(768);
  for (let i = 0; i < 768; i++) orig[i] = (i % 256) * 257;
  const { lut, clamped, effectiveTemperature } = buildSafeLut(orig, 2700, 100);
  assert.equal(clamped, true);
  const bMax = Math.max(...Array.from(lut.slice(512)));
  assert.ok(bMax >= 32768, `B max=${bMax} 应 ≥ 32768`);
  assert.ok(effectiveTemperature > 2700, `生效色温 ${effectiveTemperature} 应高于请求 2700`);
  assert.ok(effectiveTemperature <= 3400, `生效色温 ${effectiveTemperature} 应在 3400 以内（实测边界≈3230）`);
});

test('buildSafeLut: 2700K+50%（双重压低）同样被钳制且不越界', () => {
  const orig = new Uint16Array(768);
  for (let i = 0; i < 768; i++) orig[i] = (i % 256) * 257;
  const { lut, clamped } = buildSafeLut(orig, 2700, 50);
  assert.equal(clamped, true);
  for (const c of [0, 1, 2]) {
    const cMax = Math.max(...Array.from(lut.slice(c * 256, c * 256 + 256)));
    assert.ok(cMax >= 32768, `通道${c} max=${cMax} 应 ≥ 32768`);
  }
});

test('buildSafeLut: 安全组合下 4500K 的读值仍≈期望（钳制不误伤正常范围）', () => {
  const orig = new Uint16Array(768).fill(65535);
  const { lut } = buildSafeLut(orig, 4500, 100);
  approx(lut[255], 65535, 1, 'R[255]');
  approx(lut[512 + 255], Math.round(65535 * temperatureGain(4500).b), 1, 'B[255]');
});
