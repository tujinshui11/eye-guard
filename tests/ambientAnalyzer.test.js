'use strict';

// W8-T8.1/T8.2（RED）：感光分析器 LumaAnalyzer 行为锁定
// 期望值独立取自设计文档 §6 感光监测（docs/superpowers/specs/2026-10-01--v2-design.md）
// 采样间隔模拟：now 逐次 +60s（60000ms）。
const { test } = require('node:test');
const assert = require('node:assert/strict');

const { LumaAnalyzer } = require('../src/main/ambient');

const STEP = 60_000; // 采样间隔 60s
const T0 = 1_700_000_000_000; // 大基值，确保首个 darker 不被 cooldown(lastDarkerAt=0) 误伤

/** 顺序喂样器：每次 feed 自动推进 now，可手动跳转 now */
function feeder(analyzer, start = T0, step = STEP) {
  let t = start;
  return {
    feed(luma) {
      const ev = analyzer.feed(luma, t);
      t += step;
      return ev;
    },
    forward(ms) {
      t += ms;
    },
    at() {
      return t;
    }
  };
}

test('T8.1 恒定光照：喂 100 次恒值不触发任何事件', () => {
  const a = new LumaAnalyzer();
  const f = feeder(a);
  for (let i = 0; i < 100; i++) {
    assert.equal(f.feed(100), null, `第 ${i} 次不应有事件`);
  }
  assert.equal(a.getState().state, 'normal');
  const st = a.getState();
  assert.equal(typeof st.smooth, 'number');
  assert.equal(typeof st.baseline, 'number');
  assert.equal(st.samples, 100);
});

test('T8.1 骤降触发：稳定 100 后骤降 50 持续 → 出现 darker', () => {
  const a = new LumaAnalyzer();
  const f = feeder(a);
  const events = [];
  for (let i = 0; i < 20; i++) events.push(f.feed(100));
  for (let i = 0; i < 30; i++) events.push(f.feed(50));
  assert.ok(
    events.includes('darker'),
    `骤降应触发 darker，实际事件序列：${JSON.stringify(events)}`
  );
  assert.equal(a.getState().state, 'dark');
});

test('T8.1 降幅不足阈值不触发：100 → 80（降 20% < 35%）无 darker', () => {
  const a = new LumaAnalyzer();
  const f = feeder(a);
  const events = [];
  for (let i = 0; i < 20; i++) events.push(f.feed(100));
  for (let i = 0; i < 30; i++) events.push(f.feed(80));
  assert.ok(
    !events.includes('darker'),
    `降幅不足不应触发 darker，实际事件序列：${JSON.stringify(events)}`
  );
  assert.equal(a.getState().state, 'normal');
});

test('T8.2 回升恢复：darker 后回到 100 → recovered', () => {
  const a = new LumaAnalyzer();
  const f = feeder(a);
  for (let i = 0; i < 20; i++) f.feed(100);

  let sawDarker = false;
  for (let i = 0; i < 30 && !sawDarker; i++) {
    if (f.feed(50) === 'darker') sawDarker = true;
  }
  assert.ok(sawDarker, '前置条件：应先出现 darker');

  let sawRecovered = false;
  for (let i = 0; i < 30 && !sawRecovered; i++) {
    if (f.feed(100) === 'recovered') sawRecovered = true;
  }
  assert.ok(sawRecovered, '回升应触发 recovered');
  assert.equal(a.getState().state, 'normal');
});

test('T8.2 冷却：冷却期内再次骤降不重复 darker，超时后可再触发', () => {
  const a = new LumaAnalyzer({ cooldownMs: 900_000 });
  const f = feeder(a);
  for (let i = 0; i < 30; i++) f.feed(100);

  // 第一次 darker
  let first = false;
  for (let i = 0; i < 30 && !first; i++) {
    if (f.feed(50) === 'darker') first = true;
  }
  assert.ok(first, '前置条件：应出现首个 darker');

  // 恢复
  let rec = false;
  for (let i = 0; i < 30 && !rec; i++) {
    if (f.feed(100) === 'recovered') rec = true;
  }
  assert.ok(rec, '前置条件：应恢复');

  // 冷却期内再次骤降 → 无第二个 darker，但状态仍转入 dark
  const suppressed = [];
  for (let i = 0; i < 30; i++) suppressed.push(f.feed(50));
  assert.ok(
    !suppressed.includes('darker'),
    `冷却期内不应重复 darker，实际序列：${JSON.stringify(suppressed)}`
  );
  assert.equal(a.getState().state, 'dark', '冷却内应仍转入 dark，仅事件被抑制');

  // 再次恢复，并推进 now 越过 cooldownMs
  let rec2 = false;
  for (let i = 0; i < 30 && !rec2; i++) {
    if (f.feed(100) === 'recovered') rec2 = true;
  }
  assert.ok(rec2, '第二次恢复应成功');
  f.forward(1_000_000); // 远超 cooldownMs(900000)

  const again = [];
  for (let i = 0; i < 30; i++) again.push(f.feed(50));
  assert.ok(
    again.includes('darker'),
    `冷却结束后应能再次触发 darker，实际序列：${JSON.stringify(again)}`
  );
});

test('T8.2 基线不吸黑：dark 期间喂暗值基线不变，回升仍能 recovered', () => {
  const a = new LumaAnalyzer();
  const f = feeder(a);
  for (let i = 0; i < 30; i++) f.feed(100);

  let dark = false;
  for (let i = 0; i < 30 && !dark; i++) {
    if (f.feed(40) === 'darker') dark = true;
  }
  assert.ok(dark, '前置条件：应先出现 darker');

  const baselineAtDark = a.getState().baseline;

  // dark 期间喂 40 次暗值
  for (let i = 0; i < 40; i++) f.feed(40);
  assert.equal(a.getState().state, 'dark');
  assert.equal(
    a.getState().baseline,
    baselineAtDark,
    'dark 期间基线不得吸收暗值'
  );

  // 回升 → recovered
  let rec = false;
  for (let i = 0; i < 30 && !rec; i++) {
    if (f.feed(100) === 'recovered') rec = true;
  }
  assert.ok(rec, '回升应能正常 recovered');
});
