'use strict';

// W3-T3.1（RED）：SettingsStore 持久化——默认值/损坏自愈/深合并/原子写
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const os = require('os');
const path = require('path');

const { SettingsStore, DEFAULTS } = require('../src/main/settings');

function tmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'eyeguard-settings-'));
}

test('load: 无文件时返回完整默认值', () => {
  const store = new SettingsStore({ dataDir: tmpDir() });
  const data = store.load();
  assert.equal(data.temperature, DEFAULTS.temperature);
  assert.equal(data.breaks.workSeconds, 1200); // AAO 20-20-20 对齐（2026-10-01 规格变更）
  assert.equal(data.brightness, 100);
});

test('load: 损坏 JSON 自愈为默认值（不抛异常）', () => {
  const dir = tmpDir();
  fs.writeFileSync(path.join(dir, 'settings.json'), '{broken json!!!');
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.temperature, DEFAULTS.temperature);
});

test('load: 部分字段缺失时补默认（深合并）', () => {
  const dir = tmpDir();
  fs.writeFileSync(
    path.join(dir, 'settings.json'),
    JSON.stringify({ temperature: 4500, breaks: { workSeconds: 777 } })
  );
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.temperature, 4500, '用户值保留');
  assert.equal(data.breaks.workSeconds, 777, '用户值保留（与默认值 1200 区分）');
  assert.equal(data.breaks.breakSeconds, 20, '缺失字段补默认');
  assert.equal(data.brightness, 100, '缺失字段补默认');
});

test('load: 未知字段保留（向前兼容）', () => {
  const dir = tmpDir();
  fs.writeFileSync(
    path.join(dir, 'settings.json'),
    JSON.stringify({ temperature: 4500, futureField: { x: 1 } })
  );
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.deepEqual(data.futureField, { x: 1 });
});

test('save: 合并 patch 并落盘（可读回）', () => {
  const dir = tmpDir();
  const store = new SettingsStore({ dataDir: dir });
  store.load();
  store.save({ temperature: 3400 });
  store.save({ breaks: { enabled: false } });

  const raw = JSON.parse(fs.readFileSync(path.join(dir, 'settings.json'), 'utf-8'));
  assert.equal(raw.temperature, 3400);
  assert.equal(raw.breaks.enabled, false);
  assert.equal(raw.breaks.workSeconds, 1200, '未提供的子字段保留（默认值）');
});

test('save: 无临时文件残留（原子写清理）', () => {
  const dir = tmpDir();
  const store = new SettingsStore({ dataDir: dir });
  store.load();
  store.save({ temperature: 5000 });
  const leftovers = fs.readdirSync(dir).filter((f) => f.endsWith('.tmp'));
  assert.equal(leftovers.length, 0);
});

// W6-T6.4（RED→GREEN）：theme/modeId 新字段 + v1 preset→v2 modeId 迁移

test('load: 无文件时 theme/modeId 取 v2 默认值', () => {
  const store = new SettingsStore({ dataDir: tmpDir() });
  const data = store.load();
  assert.equal(data.theme, 'deepsea');
  assert.equal(data.modeId, 'natural');
  assert.equal(DEFAULTS.theme, 'deepsea');
  assert.equal(DEFAULTS.modeId, 'natural');
});

/** 构造一份 v1 配置（无 modeId 字段，含 preset）并返回加载结果 */
function loadV1(preset) {
  const dir = tmpDir();
  fs.writeFileSync(
    path.join(dir, 'settings.json'),
    JSON.stringify({ temperature: 4500, preset, breaks: { workSeconds: 777 } })
  );
  const store = new SettingsStore({ dataDir: dir });
  return { store, data: store.load() };
}

test('迁移: preset "off" → modeId "natural"', () => {
  const { data } = loadV1('off');
  assert.equal(data.modeId, 'natural');
});

test('迁移: preset "office" → modeId "office"（同名）', () => {
  const { store, data } = loadV1('office');
  assert.equal(data.modeId, 'office');
  assert.equal(store.get().modeId, 'office', 'get() 可见迁移结果');
  assert.equal(data.preset, 'office', 'v1 preset 字段保留不动');
  assert.equal(data.temperature, 4500, '其它字段不受迁移影响');
  assert.equal(data.breaks.workSeconds, 777, '深合并路径不受迁移影响');
});

test('迁移: preset "evening" → modeId "evening"（同名）', () => {
  const { data } = loadV1('evening');
  assert.equal(data.modeId, 'evening');
});

test('迁移: preset "night" → modeId "night"（同名）', () => {
  const { data } = loadV1('night');
  assert.equal(data.modeId, 'night');
});

test('迁移: preset "deepnight" → modeId "deepnight"（同名）', () => {
  const { data } = loadV1('deepnight');
  assert.equal(data.modeId, 'deepnight');
});

test('迁移: preset "custom" → modeId "custom"', () => {
  const { data } = loadV1('custom');
  assert.equal(data.modeId, 'custom');
});

test('迁移: 未知 preset 值回落 modeId "natural"', () => {
  const { data } = loadV1('some-future-preset');
  assert.equal(data.modeId, 'natural');
});

test('迁移: 无 preset 字段时不迁移（保持默认 natural）', () => {
  const dir = tmpDir();
  fs.writeFileSync(path.join(dir, 'settings.json'), JSON.stringify({ temperature: 4500 }));
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.modeId, 'natural');
  assert.equal(data.temperature, 4500);
});

test('迁移: 磁盘已有 modeId 时不被 preset 覆盖', () => {
  const dir = tmpDir();
  fs.writeFileSync(
    path.join(dir, 'settings.json'),
    JSON.stringify({ preset: 'night', modeId: 'custom' })
  );
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.modeId, 'custom', '显式 modeId 优先于 preset 映射');
  assert.equal(store.get().modeId, 'custom');
});

test('迁移: 损坏 JSON 仍自愈为默认值（迁移不影响自愈路径）', () => {
  const dir = tmpDir();
  fs.writeFileSync(path.join(dir, 'settings.json'), '{broken json!!!');
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.modeId, 'natural');
  assert.equal(data.theme, 'deepsea');
});

test('迁移: 端到端——老配置加载后 save，迁移值随内存态落盘', () => {
  const dir = tmpDir();
  // 老用户真实配置文件：v1 字段齐备、无 modeId / theme
  fs.writeFileSync(
    path.join(dir, 'settings.json'),
    JSON.stringify({ preset: 'night', temperature: 4200, brightness: 88, breaks: { workSeconds: 900 } })
  );

  const store = new SettingsStore({ dataDir: dir });
  const loaded = store.load();
  assert.equal(loaded.modeId, 'night', '加载即迁移（get/返回值可见）');
  assert.equal(loaded.theme, 'deepsea', '新字段补默认');
  assert.equal(loaded.temperature, 4200, '用户既有值保留');
  assert.equal(loaded.breaks.workSeconds, 900, '深合并保留用户子字段');

  store.save({ brightness: 70 });
  const disk = JSON.parse(fs.readFileSync(path.join(dir, 'settings.json'), 'utf-8'));
  assert.equal(disk.modeId, 'night', '迁移结果随 save 持久化');
  assert.equal(disk.theme, 'deepsea');
  assert.equal(disk.brightness, 70, 'patch 生效');
  assert.equal(disk.temperature, 4200, '既有值未被 patch 破坏');
  assert.equal(disk.preset, 'night', 'v1 preset 原样保留（未做删除）');
});
