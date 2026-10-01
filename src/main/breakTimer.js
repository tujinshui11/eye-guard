'use strict';

/**
 * BreakTimer：休息提醒状态机（design.md:194）
 *
 *   working --到时--> alerting --[开始休息]--> resting --到时--> working
 *      ^                |--[推迟]--> working(短计时)
 *      |                |--[跳过]--> working(完整计时)
 *      +--[暂停]--< [恢复]（暂停期间不触发；恢复后按剩余时间继续）
 *
 * 时间戳基准（非 tick 累计）：睡眠/唤醒、时钟跳跃后仍正确。
 * 由外部定时器（生产：setInterval 1s）驱动 tick()；测试注入假时钟。
 */

class BreakTimer {
  /**
   * @param {object} opts
   * @param {() => number} [opts.now] 时钟（ms），测试可注入
   * @param {number} [opts.postponeSeconds] 推迟时长（默认 300s）
   * @param {(e: {type: string, [k: string]: any}) => void} [opts.onEvent] 状态事件
   */
  constructor({ now = () => Date.now(), postponeSeconds = 300, onEvent } = {}) {
    this.now = now;
    this.postponeSeconds = postponeSeconds;
    this.onEvent = onEvent || (() => {});
    this.config = { workSeconds: 2400, breakSeconds: 300 };

    this.state = 'idle';
    this.phaseEndTs = null;
    this._pausedState = null;
    this._pausedRemainMs = null;
    this.pausedUntilTs = null;
  }

  configure({ workSeconds, breakSeconds } = {}) {
    if (Number.isFinite(workSeconds) && workSeconds > 0) this.config.workSeconds = workSeconds;
    if (Number.isFinite(breakSeconds) && breakSeconds > 0) this.config.breakSeconds = breakSeconds;
    if (this.state === 'working' && this.phaseEndTs) {
      // 运行中调整：按新时长重排当前阶段（从本阶段开始时刻算）
      const elapsedMs = this.config.workSeconds * 1000 - Math.max(0, this.phaseEndTs - this.now());
      const newEnd = this.now() + Math.max(1000, this.config.workSeconds * 1000 - Math.max(0, elapsedMs));
      this.phaseEndTs = newEnd;
    }
  }

  start() {
    this._enterWorking();
  }

  /** 停止（用于退出/禁用场景） */
  stop() {
    this.state = 'idle';
    this.phaseEndTs = null;
    this._pausedState = null;
    this._pausedRemainMs = null;
    this.pausedUntilTs = null;
  }

  _enterWorking(seconds = this.config.workSeconds) {
    this.state = 'working';
    this.phaseEndTs = this.now() + seconds * 1000;
    this.onEvent({ type: 'working', phaseEndTs: this.phaseEndTs });
  }

  _enterAlerting() {
    this.state = 'alerting';
    this.phaseEndTs = null;
    this.onEvent({ type: 'alerting' });
  }

  /** alerting → resting（开始休息） */
  beginRest() {
    if (this.state !== 'alerting') return false;
    this.state = 'resting';
    this.phaseEndTs = this.now() + this.config.breakSeconds * 1000;
    this.onEvent({ type: 'resting', phaseEndTs: this.phaseEndTs });
    return true;
  }

  /** alerting → working（推迟 postponeSeconds） */
  postpone() {
    if (this.state !== 'alerting') return false;
    this._enterWorking(this.postponeSeconds);
    this.onEvent({ type: 'postponed' });
    return true;
  }

  /** alerting/resting → working（完整工作计时） */
  skip() {
    if (this.state !== 'alerting' && this.state !== 'resting') return false;
    this._enterWorking(this.config.workSeconds);
    this.onEvent({ type: 'skipped' });
    return true;
  }

  /**
   * 暂停提醒
   * @param {number|null} untilTs 自动恢复时间戳；null = 手动恢复
   */
  pause(untilTs = null) {
    if (this.state === 'paused' || this.state === 'idle') return false;
    this._pausedState = this.state;
    this._pausedRemainMs = this.phaseEndTs ? Math.max(0, this.phaseEndTs - this.now()) : null;
    this.state = 'paused';
    this.pausedUntilTs = untilTs || null;
    this.onEvent({ type: 'paused', until: this.pausedUntilTs });
    return true;
  }

  resume() {
    if (this.state !== 'paused') return false;
    const back = this._pausedState || 'working';
    this.state = back;
    if (this._pausedRemainMs !== null) {
      this.phaseEndTs = this.now() + this._pausedRemainMs;
    }
    this._pausedState = null;
    this.pausedUntilTs = null;
    this.onEvent({ type: 'resumed', state: this.state });
    return true;
  }

  /** 外部定时器驱动（建议 1s 间隔）；时间戳基准，tick 频率不影响正确性 */
  tick() {
    if (this.state === 'paused') {
      if (this.pausedUntilTs && this.now() >= this.pausedUntilTs) this.resume();
      return;
    }
    if (this.phaseEndTs && this.now() >= this.phaseEndTs) {
      if (this.state === 'working') this._enterAlerting();
      else if (this.state === 'resting') this._enterWorking();
    }
  }

  /** @returns {{ state: string, remainingSeconds: number|null, pausedUntil: number|null }} */
  getState() {
    const counting = this.state === 'working' || this.state === 'resting';
    const remainingSeconds =
      counting && this.phaseEndTs
        ? Math.max(0, Math.ceil((this.phaseEndTs - this.now()) / 1000))
        : null;
    return { state: this.state, remainingSeconds, pausedUntil: this.pausedUntilTs };
  }
}

module.exports = { BreakTimer };
