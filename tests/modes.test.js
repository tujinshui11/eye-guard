'use strict';

// W6-T6.3：模式表行为锁定
// 期望值独立取自设计文档 §4.2 模式表（docs/superpowers/specs/2026-10-01--v2-design.md）
// 与调研结论（docs/eye-parameters-research.md 一、三节），不从实现反推。
const { test } = require('node:test');
const assert = require('node:assert/strict');

const { MODES, getMode, DEFAULT_MODE_ID } = require('../src/main/modes');

test('MODES: id 全局唯一', () => {
  const ids = MODES.map((m) => m.id);
  assert.equal(new Set(ids).size, ids.length, `存在重复 id：${ids.join(', ')}`);
});

test('MODES: 每档字段完整且类型正确（id/name 字符串，kelvin/brightness 数值）', () => {
  for (const m of MODES) {
    assert.equal(typeof m.id, 'string', `id 类型错误：${JSON.stringify(m)}`);
    assert.ok(m.id.length > 0, 'id 不得为空串');
    assert.equal(typeof m.name, 'string', `name 类型错误：${m.id}`);
    assert.ok(m.name.length > 0, `${m.id} 的 name 不得为空串`);
    assert.equal(typeof m.kelvin, 'number', `kelvin 类型错误：${m.id}`);
    assert.equal(typeof m.brightness, 'number', `brightness 类型错误：${m.id}`);
    assert.ok(Number.isInteger(m.kelvin), `${m.id} 的 kelvin 应为整数`);
    assert.ok(Number.isInteger(m.brightness), `${m.id} 的 brightness 应为整数`);
  }
});

test('MODES: 每档 kelvin ∈ [2000, 10000]（W6 暖↔冷全谱色温域）', () => {
  for (const m of MODES) {
    assert.ok(
      m.kelvin >= 2000 && m.kelvin <= 10000,
      `${m.id} kelvin=${m.kelvin} 越界 [2000, 10000]`
    );
  }
});

test('MODES: 每档 brightness ∈ [50, 100]', () => {
  for (const m of MODES) {
    assert.ok(
      m.brightness >= 50 && m.brightness <= 100,
      `${m.id} brightness=${m.brightness} 越界 [50, 100]`
    );
  }
});

test('MODES: 与 §4.2 表格逐档一致（id/kelvin/brightness）', () => {
  // [id, kelvin, brightness]，顺序即文档表格顺序
  const expected = [
    ['natural', 6500, 100],
    ['focus', 8000, 100],
    ['office', 5500, 100],
    ['reading', 5000, 90],
    ['evening', 4500, 80],
    ['night', 3400, 70],
    ['deepnight', 2700, 60]
  ];
  assert.equal(MODES.length, expected.length, `档数应为 ${expected.length}`);
  assert.deepEqual(
    MODES.map((m) => [m.id, m.kelvin, m.brightness]),
    expected
  );
});

test('getMode: 每一档都能按 id 命中且返回同值对象', () => {
  for (const m of MODES) {
    const got = getMode(m.id);
    assert.ok(got, `getMode('${m.id}') 不应为空`);
    assert.equal(got.id, m.id);
    assert.equal(got.kelvin, m.kelvin);
    assert.equal(got.brightness, m.brightness);
    assert.equal(got.name, m.name);
  }
});

test('getMode: 未命中返回 null（未知 id / 空串 / 自定义态）', () => {
  assert.equal(getMode('no-such-mode'), null);
  assert.equal(getMode(''), null);
  assert.equal(getMode('custom'), null); // 手动拖动滑块后的伪 id 不在表内
  assert.equal(getMode('NATURAL'), null); // 大小写敏感
});

test('DEFAULT_MODE_ID: 存在于模式表内，且为原色档', () => {
  const def = getMode(DEFAULT_MODE_ID);
  assert.ok(def, `DEFAULT_MODE_ID='${DEFAULT_MODE_ID}' 必须能在 MODES 中查到`);
  assert.equal(DEFAULT_MODE_ID, 'natural');
  assert.equal(def.kelvin, 6500);
  assert.equal(def.brightness, 100);
});
