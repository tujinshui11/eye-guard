'use strict';

// 休息提醒窗口交互：alerting（该休息了）/ resting（休息中）两态渲染

// 主题：跟随设置（dark 深色 / light 浅色 / warm 暖橙 / anime 胖鱼），加载时应用
const THEMES = ['dark', 'light', 'warm', 'anime'];
const applyTheme = (t) => {
  document.documentElement.dataset.theme = THEMES.includes(t) ? t : 'dark';
};
window.eyeGuard
  .getSettings()
  .then((s) => applyTheme(s && s.theme))
  .catch(() => applyTheme(null));

const titleEl = document.getElementById('title');
const subtitleEl = document.getElementById('subtitle');
const countdownEl = document.getElementById('countdown');
const actionsEl = document.getElementById('actions');
const btnSkipRest = document.getElementById('btn-skip-rest');

function fmt(seconds) {
  const s = Math.max(0, Math.floor(seconds || 0));
  const m = String(Math.floor(s / 60)).padStart(2, '0');
  const ss = String(s % 60).padStart(2, '0');
  return m + ':' + ss;
}

function render(state) {
  if (!state) return;
  if (state.state === 'resting') {
    document.body.dataset.mode = 'resting';
    titleEl.textContent = '休息中';
    subtitleEl.textContent = '看看远处，放松眼睛';
    countdownEl.textContent = fmt(state.remainingSeconds);
    countdownEl.classList.remove('hidden');
    actionsEl.classList.add('hidden');
    btnSkipRest.classList.remove('hidden');
  } else {
    document.body.dataset.mode = 'alerting';
    titleEl.textContent = '该休息一下了';
    subtitleEl.textContent = '眺望 20 英尺（约 6 米）外，放松眼睛';
    countdownEl.classList.add('hidden');
    actionsEl.classList.remove('hidden');
    btnSkipRest.classList.add('hidden');
  }
}

function action(name) {
  window.eyeGuard.breakAction(name).catch(() => {});
}

document.getElementById('btn-rest').addEventListener('click', () => action('beginRest'));
document.getElementById('btn-postpone').addEventListener('click', () => action('postpone'));
document.getElementById('btn-skip').addEventListener('click', () => action('skip'));
btnSkipRest.addEventListener('click', () => action('skip'));

window.eyeGuard.onBreakUpdate(render);

// 打开时立即拉取当前状态（防止 did-finish-load 前的事件丢失）
window.eyeGuard
  .getBreakState()
  .then((st) => render(st))
  .catch(() => {});
