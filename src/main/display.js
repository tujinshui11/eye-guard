'use strict';

/**
 * DisplayController：屏幕色彩状态控制
 *
 * 职责（设计文档 §5.2-5.4）：
 *  - 启动时存档原始 gamma ramp（内存 + 磁盘 gamma-backup.json 双备份）
 *  - apply：应用色温（安全钳制 + 写入失败夹逼回退）
 *  - restore：恢复原始色彩
 *  - dirty 标记：防崩溃后屏幕偏色滞留（下次启动自愈）
 *
 * 亮度调节不在本模块（W3 用透明遮罩实现，不占用 gamma 空间——
 * 因实测本机驱动「50% 规则」，gamma 叠加亮度会挤压色温可用范围）。
 */

const fs = require('fs');
const path = require('path');
const gammaDefault = require('./gamma');
const { buildSafeLut, SAFE_MAX_VALUE } = require('./temperature');

class DisplayController {
  /**
   * @param {object} opts
   * @param {string} opts.dataDir 持久化目录（Electron userData）
   * @param {object} [opts.gammaApi] 可注入的 gamma IO（测试注入 fake；默认真实 gdi32）
   */
  constructor({ dataDir, gammaApi = gammaDefault } = {}) {
    this.dataDir = dataDir;
    this.gamma = gammaApi;
    this.backupPath = path.join(dataDir, 'gamma-backup.json');
    this.originalRamp = null;
    this.lastGoodLut = null;
    this._dirty = false;
    /** 当前生效状态（UI 读取） */
    this.current = { temperature: 6500, enabled: true };
  }

  /**
   * 启动初始化：
   *  有备份（dirty=true）→ 先恢复原始色彩（自愈）→ 再继续
   *  无备份 → 读取当前 ramp 存档
   * @returns {{ healed: boolean }}
   */
  init() {
    const backup = this._readBackup();
    let healed = false;

    if (backup && Array.isArray(backup.ramp) && backup.ramp.length === 768) {
      this.originalRamp = Uint16Array.from(backup.ramp);
      this._dirty = !!backup.dirty;
      if (this._dirty) {
        try {
          const ok = this.gamma.writeRamp(this.originalRamp);
          healed = ok;
          console.log(
            '[display] 检测到上次异常退出（dirty=true），恢复原始色彩：' + (ok ? 'OK' : 'FAILED')
          );
        } catch (err) {
          console.warn('[display] 自愈恢复失败:', err.message);
        }
      }
      this._setDirty(false);
    } else {
      this.originalRamp = this.gamma.readRamp();
      this._dirty = false;
      this._writeBackup(false);
      console.log('[display] 首次运行：已存档原始 ramp');
    }

    return { healed };
  }

  /**
   * 应用色温（亮度固定 100%——亮度由遮罩层负责）
   * @param {number} temperature 2000-6500
   * @returns {{ ok: boolean, clamped: boolean, effectiveTemperature: number, error?: string }}
   */
  apply(temperature) {
    if (!this.originalRamp) throw new Error('DisplayController 未初始化（先调用 init）');
    if (!this.current.enabled) {
      return { ok: true, clamped: false, effectiveTemperature: 6500, skipped: true };
    }

    // 应用前先落 dirty——崩溃后下次启动才能自愈
    this._setDirty(true);

    let safe = SAFE_MAX_VALUE;
    let result = buildSafeLut(this.originalRamp, temperature, 100, safe);
    let ok = this.gamma.writeRamp(result.lut);

    // 夹逼回退：面向阈值比 32768 更严的驱动（每次提升安全边界重试）
    for (let i = 0; !ok && i < 8; i++) {
      safe += 4096;
      if (safe > 65535) break;
      result = buildSafeLut(this.originalRamp, temperature, 100, safe);
      ok = this.gamma.writeRamp(result.lut);
    }

    if (!ok) {
      // 彻底失败：回退上一有效状态（或原始值）
      try {
        this.gamma.writeRamp(this.lastGoodLut || this.originalRamp);
      } catch (err) {
        console.warn('[display] 失败回退写入异常:', err.message);
      }
      return {
        ok: false,
        clamped: true,
        effectiveTemperature: this.current.temperature,
        error: '该色温超出当前显卡驱动的支持范围'
      };
    }

    this.lastGoodLut = result.lut;
    this.current.temperature = result.effectiveTemperature;
    return {
      ok: true,
      clamped: result.clamped,
      effectiveTemperature: result.effectiveTemperature
    };
  }

  /**
   * 恢复原始色彩（并清除 dirty）
   * @returns {boolean} 是否成功
   */
  restore() {
    if (!this.originalRamp) throw new Error('DisplayController 未初始化（先调用 init）');
    const ok = this.gamma.writeRamp(this.originalRamp);
    this._setDirty(!ok); // 失败则保持 dirty，等下次启动自愈
    return ok;
  }

  /** 提取当前原始 ramp 副本（selftest/调试用） */
  getOriginalRamp() {
    return this.originalRamp ? Uint16Array.from(this.originalRamp) : null;
  }

  // ---- 备份文件 ----

  _readBackup() {
    try {
      const data = JSON.parse(fs.readFileSync(this.backupPath, 'utf-8'));
      return data && typeof data === 'object' ? data : null;
    } catch {
      return null;
    }
  }

  _setDirty(dirty) {
    if (this._dirty === dirty) return; // 状态未变不写盘（拖滑块高频调用时的节流）
    this._dirty = dirty;
    this._writeBackup(dirty);
  }

  _writeBackup(dirty) {
    if (!this.originalRamp) return;
    const payload = {
      savedAt: new Date().toISOString(),
      dirty,
      ramp: Array.from(this.originalRamp)
    };
    try {
      fs.mkdirSync(this.dataDir, { recursive: true });
      fs.writeFileSync(this.backupPath, JSON.stringify(payload));
    } catch (err) {
      console.warn('[display] 写入备份失败:', err.message);
    }
  }
}

module.exports = { DisplayController };
