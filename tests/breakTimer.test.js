'use strict';

// W4-T4.1（RED）：BreakTimer 状态机——假时钟注入，时间戳基准（跨睡眠不漂移）
const { test } = require('node:test');
const assert = require('node:assert/strict');

const { BreakTimer } = require('../src/main/breakTimer');

function makeTimer(opts = {}) {
  let t = 0;
  const events = [];
  const timer = new BreakTimer({
    now: () => t,
    postponeSeconds: 300,
    onEvent: (e) => events.push(e.type),
    ...opts
  });
  timer.configure({ workSeconds: 100, breakSeconds: 20 });
  return {
    timer,
    events,
    setT: (v) => {
      t = v;
    },
    advance: (ms) => {
      t += ms;
    }
  };
}

test('working 到时 → alerting', () => {
  const { timer, events, setT } = makeTimer();
  timer.start();
  assert.equal(timer.getState().state, 'working');

  setT(99_999);
  timer.tick();
  assert.equal(timer.getState().state, 'working');

  setT(100_000);
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
  assert.ok(events.includes('alerting'));
});

test('alerting → beginRest → resting → 到时回 working（完整计时）', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(100_000);
  timer.tick();

  assert.equal(timer.beginRest(), true);
  assert.equal(timer.getState().state, 'resting');

  setT(119_999);
  timer.tick();
  assert.equal(timer.getState().state, 'resting');

  setT(120_000);
  timer.tick();
  assert.equal(timer.getState().state, 'working');
  // 新一轮工作计时完整（100s）
  setT(120_000 + 99_999);
  timer.tick();
  assert.equal(timer.getState().state, 'working');
  setT(120_000 + 100_000);
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
});

test('postpone：推迟 300 秒后再提醒', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(100_000);
  timer.tick();

  assert.equal(timer.postpone(), true);
  assert.equal(timer.getState().state, 'working');

  setT(100_000 + 299_999);
  timer.tick();
  assert.equal(timer.getState().state, 'working');

  setT(100_000 + 300_000);
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
});

test('skip：跳过提醒回到完整工作计时', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(100_000);
  timer.tick();

  assert.equal(timer.skip(), true);
  setT(100_000 + 99_999);
  timer.tick();
  assert.equal(timer.getState().state, 'working');
  setT(100_000 + 100_000);
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
});

test('pause：暂停期间不触发；恢复后按剩余时间继续', () => {
  const { timer, setT } = makeTimer();
  timer.start();

  setT(50_000);
  timer.pause(50_000 + 3_600_000); // 暂停至 1 小时后
  assert.equal(timer.getState().state, 'paused');

  // 时间大跳（跨过原工作到点时刻与暂停截止时刻）
  setT(50_000 + 3_600_001);
  timer.tick();
  assert.equal(timer.getState().state, 'working', '暂停到点自动恢复');

  // 剩余 50s：3999 后仍在工作，第 50s 到 alert
  const resumeAt = 50_000 + 3_600_001;
  setT(resumeAt + 49_999);
  timer.tick();
  assert.equal(timer.getState().state, 'working');
  setT(resumeAt + 50_000);
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
});

test('resume：手动恢复后时钟跳跃不会产生负剩余', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(50_000);
  timer.pause(null); // 无截止时间的手动暂停
  setT(9_999_999);
  timer.tick();
  assert.equal(timer.getState().state, 'paused');

  assert.equal(timer.resume(), true);
  const st = timer.getState();
  assert.equal(st.state, 'working');
  assert.ok(st.remainingSeconds >= 0, `剩余 ${st.remainingSeconds}`);
  assert.ok(st.remainingSeconds <= 50, '剩余应约 50s');
});

test('长时间睡眠：直接大跳到超过工作时长 → 立即 alert（无异常）', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(0);
  setT(30_000_000); // 睡 8 小时后唤醒
  timer.tick();
  assert.equal(timer.getState().state, 'alerting');
});

test('skip：resting 中跳过休息 → 回到工作计时', () => {
  const { timer, setT } = makeTimer();
  timer.start();
  setT(100_000);
  timer.tick();
  timer.beginRest();
  assert.equal(timer.getState().state, 'resting');

  assert.equal(timer.skip(), true);
  assert.equal(timer.getState().state, 'working');
});
