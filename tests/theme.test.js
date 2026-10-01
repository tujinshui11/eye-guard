'use strict';

// 主题契约锁定：可选主题必须在「面板按钮 / renderer 白名单 / 三窗口 CSS」三处同步。
// 背景：主题横跨 index/break/propose 三个渲染入口，新增主题若只改 index.*，
// break/propose 弹窗会静默回退默认皮肤——本测试防止该缺口。
// 注：Rust 重写版裁剪——原「anime 经 SettingsStore 读回」用例依赖已退役的
// Electron 侧 settings；持久化闭环现由 Rust settings 测试覆盖。
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf-8');

/** 全部可选主题 id（deepsea 为 :root 默认值，不需要显式覆盖块） */
const THEMES = ['deepsea', 'warm', 'anime'];
const OVERRIDE_THEMES = THEMES.filter((t) => t !== 'deepsea');

const CSS_FILES = [
  'src/renderer/index.css',
  'src/renderer/break.css',
  'src/renderer/propose.css'
];

test('主题：三个窗口 CSS 均为每个可切换主题定义覆盖块', () => {
  for (const f of CSS_FILES) {
    const css = read(f);
    for (const t of OVERRIDE_THEMES) {
      assert.ok(css.includes(`[data-theme="${t}"]`), `${f} 缺少主题覆盖块：${t}`);
    }
  }
});

test('主题：设置面板按钮集合 === 主题全集', () => {
  const html = read('src/renderer/index.html');
  const found = [...html.matchAll(/class="theme-btn[^"]*"\s+data-theme="([^"]+)"/g)].map((m) => m[1]);
  assert.deepEqual([...found].sort(), [...THEMES].sort(), `按钮与主题全集不一致：${found.join(', ')}`);
});

test('主题：applyTheme 由 THEMES 白名单驱动，集合与主题全集一致', () => {
  const js = read('src/renderer/index.js');
  const arr = js.match(/const THEMES = \[([^\]]+)\]/);
  assert.ok(arr, '未找到 THEMES 白名单数组');
  const ids = [...arr[1].matchAll(/'([^']+)'/g)].map((m) => m[1]);
  assert.deepEqual(ids.sort(), [...THEMES].sort(), `白名单与主题全集不一致：${ids.join(', ')}`);
  assert.ok(/THEMES\.includes\(theme\)/.test(js), 'applyTheme 未用 THEMES 白名单过滤未知值');
});
