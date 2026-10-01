'use strict';

// 护眼助手 · 设置面板交互（v2）
// 深海/暖橙主题 · 场景模式网格 · 暖↔冷精细调节 · 休息提醒 · 通用设置

const $ = (id) => document.getElementById(id);

const tempSlider = $('temperature');
const tempValue = $('temperature-value');
const tempNote = $('temp-note');
const brightSlider = $('brightness');
const brightValue = $('brightness-value');
const modeGrid = $('mode-grid');
const statusEl = $('status');
const versionEl = $('version');

const breakEnabled = $('break-enabled');
const breakWork = $('break-work');
const breakRest = $('break-rest');
const breakStyle = $('break-style');
const breakPause = $('break-pause');
const breakStatus = $('break-status');
const breakNote = $('break-note');

const autoLaunch = $('auto-launch');

// 模式色点（与各自色温语义对应：暖 → 冷）
const MODE_SWATCH = {
  natural: '#e8ecf8',
  focus: '#a9c7ff',
  office: '#f2ead9',
  reading: '#ffe6c9',
  evening: '#ffd9ac',
  night: '#ffc98f',
  deepnight: '#ffb46b'
};

let modesCache = [];
let currentModeId = 'natural';
let draggingTemp = false;
let draggingBright = false;
let breakPaused = false;

// ---- 小工具 ----

function showNote(el, text) {
  if (!el) return;
  if (text) {
    el.textContent = text;
    el.classList.remove('hidden');
  } else {
    el.textContent = '';
    el.classList.add('hidden');
  }
}

/** 尾部节流：拖动滑轨时实时预览但不轰炸 IPC */
function makeThrottle(fn, ms) {
  let last = 0;
  let timer = null;
  let lastArgs = null;
  return function (...args) {
    lastArgs = args;
    const now = Date.now();
    const run = () => {
      last = Date.now();
      timer = null;
      fn(...lastArgs);
    };
    if (now - last >= ms) run();
    else if (!timer) timer = setTimeout(run, ms - (now - last));
  };
}

// ---- 主题 ----

function applyTheme(theme) {
  const t = theme === 'warm' ? 'warm' : 'deepsea';
  document.documentElement.dataset.theme = t;
  document.querySelectorAll('.theme-btn').forEach((b) => {
    b.classList.toggle('active', b.dataset.theme === t);
  });
}

document.querySelectorAll('.theme-btn').forEach((btn) => {
  btn.addEventListener('click', async () => {
    const theme = btn.dataset.theme;
    applyTheme(theme);
    try {
      await window.eyeGuard.updateSettings({ theme });
    } catch (err) {
      console.warn('[ui] 主题保存失败:', err.message);
    }
  });
});

// ---- 状态药丸 ----

function setStatus(enabled) {
  statusEl.textContent = enabled ? '已启用' : '已暂停';
  statusEl.classList.toggle('off', !enabled);
}

// ---- 场景模式 ----

function renderModes() {
  modeGrid.innerHTML = '';
  for (const m of modesCache) {
    const chip = document.createElement('button');
    chip.className = 'mode-chip' + (m.id === currentModeId ? ' active' : '');
    chip.dataset.mode = m.id;

    const dot = document.createElement('span');
    dot.className = 'mode-dot';
    dot.style.background = MODE_SWATCH[m.id] || '#e8ecf8';

    const name = document.createElement('span');
    name.className = 'mode-name';
    name.textContent = m.name;

    const sub = document.createElement('span');
    sub.className = 'mode-sub';
    sub.textContent = m.kelvin + 'K · ' + m.brightness + '%';

    chip.append(dot, name, sub);
    chip.addEventListener('click', () => requestMode(m.id));
    modeGrid.appendChild(chip);
  }
}

async function requestMode(id) {
  try {
    const res = await window.eyeGuard.applyMode(id);
    if (res && res.ok) {
      currentModeId = id;
      renderModes();
      setTempUI(res.temperature);
      setBrightUI(res.brightness);
      const mode = modesCache.find((m) => m.id === id);
      if (mode && Math.abs(mode.kelvin - res.temperature) > 100) {
        showNote(tempNote, '受屏幕硬件限制，实际生效 ≈ ' + res.temperature + 'K');
      } else {
        showNote(tempNote, '');
      }
    } else {
      showNote(tempNote, (res && res.error) || '模式应用失败');
    }
  } catch (err) {
    showNote(tempNote, '调用失败：' + err.message);
  }
}

// ---- 精细调节 ----

function setTempUI(k) {
  if (typeof k !== 'number' || !Number.isFinite(k)) return;
  tempSlider.value = String(k);
  tempValue.textContent = String(k);
}

function setBrightUI(b) {
  if (typeof b !== 'number' || !Number.isFinite(b)) return;
  brightSlider.value = String(b);
  brightValue.textContent = String(b);
}

const applyTemp = makeThrottle(async (k) => {
  try {
    const res = await window.eyeGuard.setTemperature(k);
    if (res && res.ok && res.clamped) {
      showNote(tempNote, '受屏幕硬件限制，实际生效 ≈ ' + res.effectiveTemperature + 'K');
    } else {
      showNote(tempNote, '');
    }
  } catch (err) {
    showNote(tempNote, '调用失败：' + err.message);
  }
}, 60);

tempSlider.addEventListener('pointerdown', () => { draggingTemp = true; });
tempSlider.addEventListener('input', () => {
  draggingTemp = true;
  tempValue.textContent = tempSlider.value;
  if (currentModeId !== 'custom') {
    currentModeId = 'custom';
    renderModes();
  }
  applyTemp(Number(tempSlider.value));
});
tempSlider.addEventListener('change', () => {
  applyTemp(Number(tempSlider.value));
  draggingTemp = false;
});

const applyBright = makeThrottle(async (b) => {
  try {
    await window.eyeGuard.setBrightness(b);
  } catch (err) {
    console.warn('[ui] 亮度调用失败:', err.message);
  }
}, 60);

brightSlider.addEventListener('pointerdown', () => { draggingBright = true; });
brightSlider.addEventListener('input', () => {
  draggingBright = true;
  brightValue.textContent = brightSlider.value;
  if (currentModeId !== 'custom') {
    currentModeId = 'custom';
    renderModes();
  }
  applyBright(Number(brightSlider.value));
});
brightSlider.addEventListener('change', () => {
  applyBright(Number(brightSlider.value));
  draggingBright = false;
});

// ---- 休息提醒 ----

function fmtRemain(sec) {
  const s = Math.max(0, Math.floor(sec || 0));
  const m = Math.floor(s / 60);
  return m + ':' + String(s % 60).padStart(2, '0');
}

function renderBreakState(st) {
  if (!st) return;
  const map = { working: '工作中', alerting: '待休息', resting: '休息中', paused: '已暂停', idle: '未启动' };
  breakPaused = st.state === 'paused';
  breakPause.textContent = breakPaused ? '恢复提醒' : '暂停提醒 1 小时';
  let text = '当前状态：' + (map[st.state] || st.state);
  if (st.remainingSeconds != null && (st.state === 'working' || st.state === 'resting')) {
    text += ' · 剩余 ' + fmtRemain(st.remainingSeconds);
  }
  breakStatus.textContent = text;
}

async function saveBreaks(patch) {
  try {
    await window.eyeGuard.updateSettings({ breaks: patch });
  } catch (err) {
    showNote(breakNote, '保存失败：' + err.message);
  }
}

breakEnabled.addEventListener('change', () => {
  saveBreaks({ enabled: breakEnabled.checked });
});

breakWork.addEventListener('change', () => {
  const minutes = Math.max(1, Math.min(240, Number(breakWork.value) || 20));
  breakWork.value = String(minutes);
  saveBreaks({ workSeconds: Math.round(minutes * 60) });
});

breakRest.addEventListener('change', () => {
  const seconds = Math.max(5, Math.min(3600, Number(breakRest.value) || 20));
  breakRest.value = String(seconds);
  saveBreaks({ breakSeconds: Math.round(seconds) });
});

breakStyle.addEventListener('change', () => {
  saveBreaks({ style: breakStyle.value });
});

$('preset-2020').addEventListener('click', () => {
  breakWork.value = '20';
  breakRest.value = '20';
  saveBreaks({ enabled: true, workSeconds: 1200, breakSeconds: 20 });
  breakEnabled.checked = true;
  showNote(breakNote, '已应用 20-20-20（每 20 分钟休息 20 秒）');
  setTimeout(() => showNote(breakNote, ''), 2600);
});

breakPause.addEventListener('click', async () => {
  try {
    if (breakPaused) {
      await window.eyeGuard.breakAction('resume');
      showNote(breakNote, '提醒已恢复');
    } else {
      await window.eyeGuard.breakAction('pause1h');
      showNote(breakNote, '提醒已暂停 1 小时（可点「恢复提醒」提前恢复）');
    }
    setTimeout(() => showNote(breakNote, ''), 2600);
  } catch (err) {
    showNote(breakNote, '操作失败：' + err.message);
  }
});

// ---- 通用 ----

autoLaunch.addEventListener('change', async () => {
  try {
    const res = await window.eyeGuard.setAutoLaunch(autoLaunch.checked);
    autoLaunch.checked = !!(res && res.openAtLogin);
  } catch (err) {
    console.warn('[ui] 自启设置失败:', err.message);
  }
});

// ---- 主进程广播 ----

window.eyeGuard.onDisplayChanged((s) => {
  if (!s) return;
  if (!draggingTemp) setTempUI(s.temperature);
  if (!draggingBright) setBrightUI(s.brightness);
  if (s.modeId && s.modeId !== currentModeId) {
    currentModeId = s.modeId;
    renderModes();
  }
  setStatus(s.enabled !== false);
});

window.eyeGuard.onThemeChanged((theme) => applyTheme(theme));

window.eyeGuard.onBreakUpdate((st) => renderBreakState(st));

// ---- 启动装载 ----

async function boot() {
  const [versionR, settingsR, modesR, stateR, autoR, breakR] = await Promise.allSettled([
    window.eyeGuard.getVersion(),
    window.eyeGuard.getSettings(),
    window.eyeGuard.listModes(),
    window.eyeGuard.getDisplayState(),
    window.eyeGuard.getAutoLaunch(),
    window.eyeGuard.getBreakState()
  ]);

  const settings = settingsR.status === 'fulfilled' ? settingsR.value : null;
  applyTheme(settings && settings.theme);

  if (versionR.status === 'fulfilled') versionEl.textContent = 'v' + versionR.value;

  if (modesR.status === 'fulfilled' && Array.isArray(modesR.value)) {
    modesCache = modesR.value;
  }
  if (settings && settings.modeId) currentModeId = settings.modeId;
  renderModes();

  if (stateR.status === 'fulfilled' && stateR.value) {
    setTempUI(stateR.value.temperature);
    setBrightUI(stateR.value.brightness);
    if (stateR.value.modeId) {
      currentModeId = stateR.value.modeId;
      renderModes();
    }
    setStatus(stateR.value.enabled !== false);
  }

  if (settings) {
    breakEnabled.checked = settings.breaks.enabled !== false;
    breakWork.value = String(Math.round((settings.breaks.workSeconds || 1200) / 60));
    breakRest.value = String(settings.breaks.breakSeconds || 20);
    breakStyle.value = settings.breaks.style || 'gentle';
  }

  if (autoR.status === 'fulfilled' && autoR.value) {
    autoLaunch.checked = !!autoR.value.openAtLogin;
  }

  if (breakR.status === 'fulfilled') renderBreakState(breakR.value);

  document.activeElement && document.activeElement.blur();
}

window.addEventListener('DOMContentLoaded', boot);
