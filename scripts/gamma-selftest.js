'use strict';

/**
 * FFI 集成自检（真实 GPU 写入，运行中屏幕会短暂变色后复原）
 *
 * 流程：读原始 ramp → 4500K 安全写入+读回校验 → 2700K+50%（H2 越界组合，
 *       应被安全钳制后写入成功）→ 6500K 恒等恢复校验 → 恢复原始 ramp（try/finally 保证）
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
