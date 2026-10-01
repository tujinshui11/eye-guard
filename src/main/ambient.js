'use strict';

/**
 * 感光监测（ambient light sensing）分析器。
 * 设计文档 §6：docs/superpowers/specs/2026-10-01--v2-design.md
 *
 * 纯计算模块：无 I/O、无 Electron 依赖，可独立单测。
 * 输入为逐次采样的环境亮度（luma，任意尺度，通常 0-100 或 0-255），
 * 输出为状态事件（null / 'darker' / 'recovered'）。
 *
 * 职责单一：只负责「由亮度序列判定环境是否骤暗 / 已恢复」，
 * 不含计时器、采样、UI、系统通知等（由 AmbientMonitor 承担）。
 *
 * 判定模型：
 *   smooth   —— 亮度指数平滑（EMA），抑制单帧噪声；
 *   baseline —— 最近 baselineSamples 个「normal 态」平滑样本的 P75，
 *               代表近期常亮水平；dark 期间不吸收暗值（防止基线被拖低）；
 *   骤暗条件 —— smooth < baseline * (1 - dropThresholdPercent/100)；
 *   恢复条件 —— smooth > baseline * (1 - dropThresholdPercent/200)（回滞）；
 *   darker 事件受 cooldownMs 限制，避免短时间内反复告警。
 */

/** 百分位（线性插值）。入参须为已升序排序的数组。 */
function percentile(sorted, p) {
  const n = sorted.length;
  if (n === 0) return 0;
  if (n === 1) return sorted[0];
  const idx = (n - 1) * p;
  const lo = Math.floor(idx);
  const hi = Math.ceil(idx);
  if (lo === hi) return sorted[lo];
  const frac = idx - lo;
  return sorted[lo] * (1 - frac) + sorted[hi] * frac;
}

const DEFAULT_OPTIONS = {
  emaAlpha: 0.3, // EMA 平滑系数 α
  baselineSamples: 60, // 基线样本窗口长度
  dropThresholdPercent: 35, // 判定骤暗的降幅阈值（%）
  cooldownMs: 900000 // darker 事件最小间隔（15 分钟）
};

/** 基线样本不足此数时不作判定 */
const MIN_BASELINE_SAMPLES = 5;

class LumaAnalyzer {
  constructor(options = {}) {
    const opts = Object.assign({}, DEFAULT_OPTIONS, options);
    this.emaAlpha = opts.emaAlpha;
    this.baselineSamples = opts.baselineSamples;
    this.dropThresholdPercent = opts.dropThresholdPercent;
    this.cooldownMs = opts.cooldownMs;

    this.smooth = null; // 平滑亮度
    this.baseline = null; // 基线（normal 样本 P75）
    this.state = 'normal'; // 'normal' | 'dark'
    this.lastDarkerAt = 0; // 上次 darker 触发时刻（初值 0）
    this._samples = 0; // 累计采样数
    this._baselineBuffer = []; // normal 态平滑样本环形窗口
  }

  /**
   * 喂入一次亮度采样。
   * @param {number} luma 环境亮度
   * @param {number} now 采样时刻（ms）
   * @returns {null|'darker'|'recovered'}
   */
  feed(luma, now) {
    // 1) 平滑：首个样本直接取原值，后续 EMA
    if (this.smooth === null) {
      this.smooth = luma;
    } else {
      this.smooth = this.emaAlpha * luma + (1 - this.emaAlpha) * this.smooth;
    }
    this._samples += 1;

    // 2) 基线只在 normal 态吸收平滑样本；dark 期间不吸收暗值
    if (this.state === 'normal') {
      this._baselineBuffer.push(this.smooth);
      if (this._baselineBuffer.length > this.baselineSamples) {
        this._baselineBuffer.shift();
      }
    }

    // 3) 样本不足，不判定
    if (this._baselineBuffer.length < MIN_BASELINE_SAMPLES) {
      return null;
    }

    // 4) 基线 = normal 样本 P75
    const sorted = this._baselineBuffer.slice().sort((x, y) => x - y);
    this.baseline = percentile(sorted, 0.75);

    const drop = this.dropThresholdPercent;
    const darkThreshold = this.baseline * (1 - drop / 100);
    const recoverThreshold = this.baseline * (1 - drop / 200);

    // 5) 状态机
    if (this.state === 'normal') {
      if (this.smooth < darkThreshold) {
        this.state = 'dark';
        // 冷却期内仍转入 dark，但不重复发 darker 事件
        if (now - this.lastDarkerAt >= this.cooldownMs) {
          this.lastDarkerAt = now;
          return 'darker';
        }
        return null;
      }
      return null;
    }

    // state === 'dark'
    if (this.smooth > recoverThreshold) {
      this.state = 'normal';
      return 'recovered'; // recovered 不受冷却限制
    }
    return null;
  }

  /** 供 UI / 调试的只读快照 */
  getState() {
    return {
      smooth: this.smooth,
      baseline: this.baseline,
      state: this.state,
      samples: this._samples
    };
  }
}

module.exports = { LumaAnalyzer, DEFAULT_OPTIONS, percentile };
