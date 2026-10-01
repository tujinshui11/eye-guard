'use strict';

// ALS（环境光传感器）模块契约：
// - parseAlsOutput: PowerShell 输出 → { available, lux }
// - ALS_SCRIPT: 必须通过 WinRT LightSensor API 读取（含 GetCurrentReading）
// 注意：本机无 ALS 硬件，硬件正路径无法实机验证（探测会返回 ALS_NONE）——
// 测试用固定输出样本锁定解析行为，探测脚本用标记断言锁定 API 调用面。
const { test } = require('node:test');
const assert = require('node:assert/strict');

const als = require('../src/main/als');

test('parseAlsOutput: ALS_NONE → 无硬件', () => {
  assert.deepEqual(als.parseAlsOutput('ALS_NONE'), { available: false, lux: null });
});

test('parseAlsOutput: ALS_LUX=123 → 可用 + 读数', () => {
  assert.deepEqual(als.parseAlsOutput('ALS_LUX=123'), { available: true, lux: 123 });
});

test('parseAlsOutput: ALS_LUX=0 也是有效读数（全黑环境）', () => {
  assert.deepEqual(als.parseAlsOutput('ALS_LUX=0'), { available: true, lux: 0 });
});

test('parseAlsOutput: 支持小数 lux', () => {
  assert.deepEqual(als.parseAlsOutput('ALS_LUX=12.6'), { available: true, lux: 12.6 });
});

test('parseAlsOutput: ALS_NO_READING → 硬件在但本次无读数', () => {
  assert.deepEqual(als.parseAlsOutput('ALS_NO_READING'), { available: true, lux: null });
});

test('parseAlsOutput: 混入杂音仍能解析出读数', () => {
  assert.deepEqual(als.parseAlsOutput('WARNING: xxx\nALS_LUX=42\n'), { available: true, lux: 42 });
});

test('parseAlsOutput: 空输出/垃圾输出 → 不可用（回落摄像头）', () => {
  assert.deepEqual(als.parseAlsOutput(''), { available: false, lux: null });
  assert.deepEqual(als.parseAlsOutput('garbage'), { available: false, lux: null });
  assert.deepEqual(als.parseAlsOutput(null), { available: false, lux: null });
});

test('ALS_SCRIPT: 调用 WinRT LightSensor API（GetDefault + GetCurrentReading）', () => {
  assert.ok(als.ALS_SCRIPT.includes('Windows.Devices.Sensors.LightSensor'), '应引用 WinRT LightSensor');
  assert.ok(als.ALS_SCRIPT.includes('GetDefault'), '应调用 GetDefault()');
  assert.ok(als.ALS_SCRIPT.includes('GetCurrentReading'), '应调用 GetCurrentReading()');
  assert.ok(als.ALS_SCRIPT.includes("'ALS_NONE'"), '无硬件时应输出 ALS_NONE 标记');
});
