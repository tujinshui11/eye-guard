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
  breaks: { enabled: true, workSeconds: 2400, breakSeconds: 300, style: 'gentle' },
  hotkeys: { tempUp: 'Control+Alt+Up', tempDown: 'Control+Alt+Down' },
  autoLaunch: false,
  pausedUntil: null
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

class SettingsStore {
  constructor({ dataDir }) {
    this.file = path.join(dataDir, 'settings.json');
    this.data = deepClone(DEFAULTS);
  }

  /** 读取并深合并到默认值（文件缺失/损坏均回落默认值，不抛异常） */
  load() {
    try {
      const raw = JSON.parse(fs.readFileSync(this.file, 'utf-8'));
      this.data = deepMerge(deepClone(DEFAULTS), isPlainObject(raw) ? raw : {});
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
