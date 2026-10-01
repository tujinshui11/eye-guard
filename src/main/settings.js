'use strict';

/**
 * SettingsStore：配置持久化（userData/settings.json）
 *  - 损坏自愈（解析失败回落默认值）
 *  - 深合并（补默认字段、保留用户字段与未知字段——向前兼容）
 *  - 原子写（临时文件 + rename）
 */

const fs = require('fs');
const path = require('path');

const DEFAULTS = {
  enabled: true,
  temperature: 6500,
  brightness: 100,
  preset: 'off',
  modeId: 'natural',
  theme: 'deepsea',
  // 休息节奏依据 AAO/AOA 20-20-20（调研：docs/eye-parameters-research.md）
  breaks: { enabled: true, workSeconds: 1200, breakSeconds: 20, style: 'gentle' },
  hotkeys: { tempUp: 'Control+Alt+Up', tempDown: 'Control+Alt+Down' },
  autoLaunch: false,
  pausedUntil: null,
  // v2：时间调度（default 三条示例；enabled 默认关，避免突然打扰）
  schedule: {
    enabled: false,
    entries: [
      { id: 's1', time: '09:00', modeId: 'office', action: 'auto', enabled: true },
      { id: 's2', time: '18:00', modeId: 'evening', action: 'ask', enabled: true },
      { id: 's3', time: '22:00', modeId: 'night', action: 'auto', enabled: true }
    ],
    lastFired: {}
  },
  // v2：感光监测（默认关闭；开启后摄像头短开测光，仅本机计算亮度，不保存不传输）
  ambient: {
    enabled: false,
    intervalSeconds: 60,
    dropThresholdPercent: 35,
    action: 'notify',
    autoModeId: 'night',
    cooldownMinutes: 15
  }
};

function isPlainObject(v) {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}

function deepClone(v) {
  if (Array.isArray(v)) return v.map(deepClone);
  if (isPlainObject(v)) {
    const o = {};
    for (const [k, val] of Object.entries(v)) o[k] = deepClone(val);
    return o;
  }
  return v;
}

function deepMerge(base, patch) {
  const out = { ...base };
  for (const [k, v] of Object.entries(patch || {})) {
    if (isPlainObject(v) && isPlainObject(base[k])) {
      out[k] = deepMerge(base[k], v);
    } else {
      out[k] = v;
    }
  }
  return out;
}

/** v1 preset → v2 modeId 映射表（未列出的未知值回落 natural） */
const PRESET_TO_MODE_ID = {
  off: 'natural',
  office: 'office',
  evening: 'evening',
  night: 'night',
  deepnight: 'deepnight',
  custom: 'custom'
};

function hasOwn(obj, key) {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

/**
 * v1→v2 迁移：仅当磁盘原始数据没有 modeId 字段、且有 preset 字段时，
 * 按 v1 preset 映射出 modeId 写入合并结果。磁盘已有 modeId 时不动。
 */
function migratePresetToModeId(raw, merged) {
  if (!isPlainObject(raw)) return merged;
  if (hasOwn(raw, 'modeId')) return merged;
  if (!hasOwn(raw, 'preset')) return merged;
  const mapped = PRESET_TO_MODE_ID[raw.preset];
  merged.modeId = mapped !== undefined ? mapped : 'natural';
  return merged;
}

class SettingsStore {
  constructor({ dataDir }) {
    this.file = path.join(dataDir, 'settings.json');
    this.data = deepClone(DEFAULTS);
  }

  /** 读取并深合并到默认值（文件缺失/损坏均回落默认值，不抛异常）；含 v1→v2 迁移 */
  load() {
    try {
      const raw = JSON.parse(fs.readFileSync(this.file, 'utf-8'));
      const disk = isPlainObject(raw) ? raw : {};
      this.data = migratePresetToModeId(disk, deepMerge(deepClone(DEFAULTS), disk));
    } catch {
      this.data = deepClone(DEFAULTS);
    }
    return this.data;
  }

  /** 当前内存中的完整配置 */
  get() {
    return this.data;
  }

  /** 合并 patch 并原子落盘 */
  save(patch) {
    this.data = deepMerge(this.data, patch || {});
    this._writeAtomic();
    return this.data;
  }

  _writeAtomic() {
    const tmp = this.file + '.tmp';
    fs.mkdirSync(path.dirname(this.file), { recursive: true });
    fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2));
    fs.renameSync(tmp, this.file);
  }
}

module.exports = { SettingsStore, DEFAULTS };
