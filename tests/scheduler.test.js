'use strict';

// W7-T7.1（RED）：时间调度判定纯函数——假时钟注入，窗口/防重/对齐逐条锁定
// 期望值独立取自设计文档 §5.2（docs/superpowers/specs/2026-10-01--v2-design.md:137-148），
// 不从实现反推。
const { test } = require('node:test');
const assert = require('node:assert/strict');

const { timeToTs, resolveDueEntry, alignStartup, markFired } = require('../src/main/scheduler');

// 假时钟：基准日 2026-10-01（本地时区）。dayOffset 便于构造昨日/明日。
// 用 Date 构造函数（而非被测的 setHours 路径）构造期望值——期望值独立于实现。
function at(dayOffset, h, mi, s = 0) {
  return new Date(2026, 9, 1 + dayOffset, h, mi, s, 0).getTime();
}

const NOW = at(0, 10, 0); // 2026-10-01 10:00:00

// ---------------------------------------------------------------- timeToTs

test('timeToTs: "HH:MM" → 基于 now 日期的今日时间戳', () => {
  assert.equal(timeToTs('09:00', NOW), at(0, 9, 0));
  assert.equal(timeToTs('10:00', NOW), NOW);
  assert.equal(timeToTs('00:00', NOW), at(0, 0, 0));
  assert.equal(timeToTs('23:59', NOW), at(0, 23, 59));
});

test('timeToTs: 基准取 now 当日，与 now 的时分无关', () => {
  // 同一 HH:MM，不同 now 时刻 → 同为该日的同一点
  assert.equal(timeToTs('09:00', at(0, 0, 1)), at(0, 9, 0));
  assert.equal(timeToTs('09:00', at(0, 23, 59)), at(0, 9, 0));
});

test('timeToTs: 非法格式一律返回 null', () => {
  const bad = ['9:00', '09:0', '24:00', '09:60', '09:00:00', '0900', '', 'abc', '0:0', ' 09:00', '09:00 ', null, undefined, 900, {}];
  for (const v of bad) {
    assert.equal(timeToTs(v, NOW), null, `非法时间 ${JSON.stringify(v)} 应返回 null`);
  }
});

// -------------------------------------------------------- resolveDueEntry

const E1 = { id: 's1', time: '09:00', modeId: 'office', action: 'auto', enabled: true };

test('resolveDueEntry: 窗口内命中 [ts, ts+window)', () => {
  assert.equal(resolveDueEntry([E1], at(0, 9, 0), {}), E1, '窗口起点 ts 应命中');
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), {}), E1);
  assert.equal(resolveDueEntry([E1], at(0, 9, 4, 59), {}), E1, '窗口终点前 1s 应命中');
});

test('resolveDueEntry: 窗口外不触发（早于 ts / 达到 ts+window）', () => {
  assert.equal(resolveDueEntry([E1], at(0, 8, 59, 59), {}), null, 'ts 前 1s 不触发');
  assert.equal(resolveDueEntry([E1], at(0, 7, 0), {}), null);
  assert.equal(resolveDueEntry([E1], at(0, 9, 5), {}), null, 'ts+window 恰为右开边界，不触发');
  assert.equal(resolveDueEntry([E1], at(0, 9, 6), {}), null);
  assert.equal(resolveDueEntry([E1], at(0, 23, 0), {}), null);
});

test('resolveDueEntry: windowMs 可调，且右开边界随之一致', () => {
  assert.equal(resolveDueEntry([E1], at(0, 9, 0, 59), {}, 60_000), E1, '1 分钟窗内(59s)命中');
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), {}, 60_000), null, 'ts+60s 右开边界不触发');
});

test('resolveDueEntry: lastFired 防重——窗内重复调用仍触发（纯函数不写 lastFired）', () => {
  // 同一输入重复调用结果稳定，且不修改传入的 lastFired
  const lf = {};
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), lf), E1);
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), lf), E1, '未更新 lastFired 时重复调用仍应命中');
  assert.deepEqual(lf, {}, 'resolveDueEntry 不得修改入参 lastFired');
});

test('resolveDueEntry: lastFired[id] >= ts 时不再触发（已触发过）', () => {
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), { s1: at(0, 9, 0) }), null, '上次触发时刻 == ts → 已触发');
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), { s1: at(0, 9, 0) + 1 }), null);
});

test('resolveDueEntry: 时钟回拨——lastFired 远大于 ts 时不重复', () => {
  // 22:00 触发过，时钟回拨到 09:00 附近
  assert.equal(resolveDueEntry([E1], at(0, 9, 1), { s1: at(0, 22, 0) }), null);
});

test('resolveDueEntry: 跨天——昨日 lastFired 不影响今日', () => {
  assert.equal(
    resolveDueEntry([E1], at(0, 9, 1), { s1: at(-1, 9, 0) }),
    E1,
    '昨日触发记录 < 今日 ts，今日应重新触发'
  );
});

test('resolveDueEntry: 多条同时 due 取 ts 最新者', () => {
  const a = { id: 'a', time: '09:00', action: 'auto', enabled: true };
  const b = { id: 'b', time: '09:03', action: 'auto', enabled: true };
  const c = { id: 'c', time: '09:01', action: 'ask', enabled: true };
  assert.equal(resolveDueEntry([a, b, c], at(0, 9, 4), {}), b, '09:03 > 09:01 > 09:00');
  assert.equal(resolveDueEntry([a, c], at(0, 9, 4), {}), c);
});

test('resolveDueEntry: enabled === false 的条目不参与', () => {
  const off = { id: 's1', time: '09:00', action: 'auto', enabled: false };
  assert.equal(resolveDueEntry([off], at(0, 9, 1), {}), null);
});

test('resolveDueEntry: 非法 time 条目被跳过，无 due 返回 null', () => {
  const bad = { id: 'x', time: '9:00', action: 'auto', enabled: true };
  assert.equal(resolveDueEntry([bad], at(0, 9, 1), {}), null);
  assert.equal(resolveDueEntry([], NOW, {}), null);
});

// ---------------------------------------------------------- alignStartup

test('alignStartup: auto 且今日已过未触发 → 补', () => {
  const e = { id: 's1', time: '09:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([e], at(0, 10, 0), {}), e);
});

test('alignStartup: auto 但今日已触发 → 不补', () => {
  const e = { id: 's1', time: '09:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([e], at(0, 10, 0), { s1: at(0, 9, 0) }), null);
});

test('alignStartup: ask 型错过窗口 → 不补弹', () => {
  const e = { id: 's2', time: '09:00', action: 'ask', enabled: true };
  assert.equal(alignStartup([e], at(0, 10, 0), {}), null);
});

test('alignStartup: 今日无已过条目 → null', () => {
  const e = { id: 's1', time: '22:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([e], at(0, 10, 0), {}), null);
  assert.equal(alignStartup([], at(0, 10, 0), {}), null);
});

test('alignStartup: 取今日已过最新条目判定，不回退到更早条目', () => {
  const older = { id: 'b', time: '08:00', action: 'auto', enabled: true };
  const newer = { id: 'a', time: '09:00', action: 'ask', enabled: true };
  // 最新（09:00）是 ask → null，即使更早的 08:00 是未触发的 auto 也不回退
  assert.equal(alignStartup([older, newer], at(0, 10, 0), {}), null);

  const newerAuto = { id: 'a', time: '09:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([older, newerAuto], at(0, 10, 0), {}), newerAuto);
});

test('alignStartup: 已过最新 auto 已触发时不回退到更早未触发条目', () => {
  const older = { id: 'b', time: '08:00', action: 'auto', enabled: true };
  const newer = { id: 'a', time: '09:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([older, newer], at(0, 10, 0), { a: at(0, 9, 0) }), null);
});

test('alignStartup: now 恰为 ts 视为已过（now >= ts）', () => {
  const e = { id: 's1', time: '10:00', action: 'auto', enabled: true };
  assert.equal(alignStartup([e], at(0, 10, 0), {}), e);
});

test('alignStartup: enabled === false 的条目不参与', () => {
  const e = { id: 's1', time: '09:00', action: 'auto', enabled: false };
  assert.equal(alignStartup([e], at(0, 10, 0), {}), null);
});

// -------------------------------------------------------------- markFired

test('markFired: 返回新对象并写入 lastFired[id]=now，不修改入参', () => {
  const orig = { keep: 111 };
  const out = markFired(orig, 's1', NOW);
  assert.deepEqual(out, { keep: 111, s1: NOW });
  assert.notEqual(out, orig, '必须返回新对象');
  assert.deepEqual(orig, { keep: 111 }, '入参不得被修改');
});

test('markFired: 覆盖同名 id 的旧值', () => {
  const out = markFired({ s1: at(-1, 9, 0) }, 's1', NOW);
  assert.equal(out.s1, NOW);
});

test('markFired: 空/缺省 lastFired 也能工作', () => {
  assert.deepEqual(markFired(null, 's1', NOW), { s1: NOW });
  assert.deepEqual(markFired({}, 's1', NOW), { s1: NOW });
});
