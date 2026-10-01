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
  assert.equal(data.breaks.workSeconds, 2400);
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
    JSON.stringify({ temperature: 4500, breaks: { workSeconds: 1200 } })
  );
  const store = new SettingsStore({ dataDir: dir });
  const data = store.load();
  assert.equal(data.temperature, 4500, '用户值保留');
  assert.equal(data.breaks.workSeconds, 1200, '用户值保留');
  assert.equal(data.breaks.breakSeconds, 300, '缺失字段补默认');
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
  assert.equal(raw.breaks.workSeconds, 2400, '未提供的子字段保留');
});

test('save: 无临时文件残留（原子写清理）', () => {
  const dir = tmpDir();
  const store = new SettingsStore({ dataDir: dir });
  store.load();
  store.save({ temperature: 5000 });
  const leftovers = fs.readdirSync(dir).filter((f) => f.endsWith('.tmp'));
  assert.equal(leftovers.length, 0);
});
