'use strict';

// 建议卡片窗口交互：调度询问（schedule）/ 感光提醒（ambient）共用
// payload: { title, body, modeName, kind: 'schedule'|'ambient' }

// 主题：跟随设置（deepsea 深海 / warm 暖橙），加载时应用
window.eyeGuard
  .getSettings()
  .then((s) => {
    document.documentElement.dataset.theme = (s && s.theme) || 'deepsea';
  })
  .catch(() => {
    document.documentElement.dataset.theme = 'deepsea';
  });

const titleEl = document.getElementById('title');
const bodyEl = document.getElementById('body');

// 60 秒无操作自动视为忽略（timeout）；用户操作后取消，避免重复上报
const TIMEOUT_MS = 60000;
let timeoutId = null;

function clearAutoClose() {
  if (timeoutId !== null) {
    clearTimeout(timeoutId);
    timeoutId = null;
  }
}

function proposeAction(name) {
  clearAutoClose();
  window.eyeGuard.proposeAction(name).catch(() => {});
}

// 渲染建议文本（textContent 填充，绝不 innerHTML 拼文本）
function render(payload) {
  if (!payload) return;
  if (typeof payload.kind === 'string') document.body.dataset.kind = payload.kind;
  if (typeof payload.title === 'string') titleEl.textContent = payload.title;
  if (typeof payload.body === 'string') bodyEl.textContent = payload.body;
}

// 主进程 → 渲染进程：推送建议内容
window.eyeGuard.onPropose(render);

// 三个动作：操作后不自关，由主进程关闭窗口
document.getElementById('btn-apply').addEventListener('click', () => proposeAction('apply'));
document.getElementById('btn-dismiss').addEventListener('click', () => proposeAction('dismiss'));
document.getElementById('btn-mute').addEventListener('click', () => proposeAction('mute'));

// 打开即启动 60 秒倒计时（超时即忽略）
timeoutId = setTimeout(() => {
  timeoutId = null;
  window.eyeGuard.proposeAction('timeout').catch(() => {});
}, TIMEOUT_MS);

// 便于测试/外部取消（渲染进程无模块系统，挂到 window）
window.__proposeClearAutoClose = clearAutoClose;
