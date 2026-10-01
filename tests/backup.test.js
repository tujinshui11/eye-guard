'use strict';

// W2-T2.6（RED）：DisplayController 备份/恢复/脏标记/钳制/失败回退
// 依赖注入：fake gamma IO + 临时目录
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const os = require('os');
const path = require('path');

const { DisplayController } = require('../src/main/display');

function stdRamp() {
  const r = new Uint16Array(768);
  for (let i = 0; i < 768; i++) r[i] = (i % 256) * 257;
  return r;
}

function makeFakeGamma(initial = stdRamp()) {
  const state = { ramp: Uint16Array.from(initial), writes: [], failAll: false };
  return {
    state,
    readRamp: () => Uint16Array.from(state.ramp),
    writeRamp(lut) {
      if (state.failAll) return false;
      state.ramp = Uint16Array.from(lut);
      state.writes.push(Uint16Array.from(lut));
      return true;
    }
  };
}

function tmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'eyeguard-test-'));
}

test('init: 首次运行——存档原始 ramp 并创建备份（dirty=false）', () => {
  const dataDir = tmpDir();
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  const res = dc.init();
  assert.equal(res.healed, false);

  const backup = JSON.parse(fs.readFileSync(path.join(dataDir, 'gamma-backup.json'), 'utf-8'));
  assert.equal(backup.dirty, false);
  assert.equal(backup.ramp.length, 768);
  assert.deepEqual(backup.ramp, Array.from(stdRamp()));
});

test('init: dirty=true 的备份触发自愈——先恢复原始色彩', () => {
  const dataDir = tmpDir();
  const backupPath = path.join(dataDir, 'gamma-backup.json');
  const original = stdRamp();
  const biased = new Uint16Array(768).fill(65535); // 模拟"上次偏色未恢复"
  fs.writeFileSync(backupPath, JSON.stringify({ savedAt: 'x', dirty: true, ramp: Array.from(original) }));

  const fake = makeFakeGamma(biased);
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  const res = dc.init();

  assert.equal(res.healed, true);
  assert.deepEqual(Array.from(fake.state.ramp), Array.from(original), '屏幕应已被恢复为原始值');
  const backup = JSON.parse(fs.readFileSync(backupPath, 'utf-8'));
  assert.equal(backup.dirty, false);
});

test('init: dirty=false 的备份不做屏幕写入', () => {
  const dataDir = tmpDir();
  fs.writeFileSync(
    path.join(dataDir, 'gamma-backup.json'),
    JSON.stringify({ savedAt: 'x', dirty: false, ramp: Array.from(stdRamp()) })
  );
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  dc.init();
  assert.equal(fake.state.writes.length, 0);
});

test('apply: 应用色温——LUT 写入设备、备份转 dirty', () => {
  const dataDir = tmpDir();
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  dc.init();

  const res = dc.apply(4500);
  assert.equal(res.ok, true);
  assert.equal(res.clamped, false);
  assert.equal(fake.state.writes.length, 1);

  const backup = JSON.parse(fs.readFileSync(path.join(dataDir, 'gamma-backup.json'), 'utf-8'));
  assert.equal(backup.dirty, true);
});

test('apply: 越界色温（2700K）自动钳制并回报生效色温', () => {
  const dataDir = tmpDir();
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  dc.init();

  const res = dc.apply(2700);
  assert.equal(res.ok, true);
  assert.equal(res.clamped, true);
  assert.ok(res.effectiveTemperature > 2700, `生效 ${res.effectiveTemperature}`);
  const bMax = Math.max(...Array.from(fake.state.ramp.slice(512)));
  assert.ok(bMax >= 32768, `B max=${bMax}`);
});

test('restore: 恢复原始色彩并清除 dirty', () => {
  const dataDir = tmpDir();
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  dc.init();
  dc.apply(4500);

  const ok = dc.restore();
  assert.equal(ok, true);
  assert.deepEqual(Array.from(fake.state.ramp), Array.from(stdRamp()));

  const backup = JSON.parse(fs.readFileSync(path.join(dataDir, 'gamma-backup.json'), 'utf-8'));
  assert.equal(backup.dirty, false);
});

test('apply: 设备持续拒绝时回退并返回 ok=false + error', () => {
  const dataDir = tmpDir();
  const fake = makeFakeGamma();
  const dc = new DisplayController({ dataDir, gammaApi: fake });
  dc.init();
  dc.apply(4500); // 先建立"上一有效状态"

  fake.state.failAll = true;
  const res = dc.apply(3400);
  assert.equal(res.ok, false);
  assert.equal(typeof res.error, 'string');
});
