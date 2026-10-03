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

// 时间调度
const scheduleEnabled = $('schedule-enabled');
const scheduleList = $('schedule-list');
const scheduleAdd = $('schedule-add');
const scheduleNote = $('schedule-note');

// 感光监测
const ambientEnabled = $('ambient-enabled');
const ambientInterval = $('ambient-interval');
const ambientThreshold = $('ambient-threshold');
const ambientAction = $('ambient-action');
const ambientMode = $('ambient-mode');
const ambientStatus = $('ambient-status');
const colorSensitiveEnabled = $('colorsensitive-enabled');
const colorSensitiveSearch = $('colorsensitive-search');
const colorSensitiveList = $('colorsensitive-list');
const colorSensitiveChosen = $('colorsensitive-chosen');
const colorSensitiveStatus = $('colorsensitive-status');

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

// 可选主题全集（applyTheme 白名单：未知值回落深海）
const THEMES = ['dark', 'light', 'warm', 'anime'];

function applyTheme(theme) {
  const t = THEMES.includes(theme) ? theme : 'dark';
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

// ---- 时间调度 ----

let settingsCache = null;

async function saveSchedule(patch) {
  try {
    const next = await window.eyeGuard.updateSettings({ schedule: patch });
    if (next) settingsCache = next;
    renderSchedule();
  } catch (err) {
    showNote(scheduleNote, '保存失败：' + err.message);
    setTimeout(() => showNote(scheduleNote, ''), 2600);
  }
}

function currentEntries() {
  const sch = (settingsCache && settingsCache.schedule) || { entries: [] };
  return JSON.parse(JSON.stringify(sch.entries || []));
}

function renderSchedule() {
  const sch = (settingsCache && settingsCache.schedule) || { enabled: false, entries: [] };
  scheduleEnabled.checked = !!sch.enabled;
  const entries = sch.entries || [];

  scheduleList.innerHTML = '';
  if (!entries.length) {
    const empty = document.createElement('p');
    empty.className = 'hint';
    empty.textContent = '暂无条目，点下方按钮添加';
    scheduleList.appendChild(empty);
    return;
  }

  entries.forEach((e, idx) => {
    const row = document.createElement('div');
    row.className = 'schedule-row' + (e.enabled === false ? ' off' : '');

    const time = document.createElement('input');
    time.type = 'time';
    time.value = e.time || '09:00';
    time.addEventListener('change', () => {
      const list = currentEntries();
      list[idx].time = time.value;
      saveSchedule({ entries: list });
    });

    const modeSel = document.createElement('select');
    for (const m of modesCache) {
      const opt = document.createElement('option');
      opt.value = m.id;
      opt.textContent = m.name;
      if (m.id === e.modeId) opt.selected = true;
      modeSel.appendChild(opt);
    }
    modeSel.addEventListener('change', () => {
      const list = currentEntries();
      list[idx].modeId = modeSel.value;
      saveSchedule({ entries: list });
    });

    const actionSel = document.createElement('select');
    for (const [v, label] of [['auto', '自动'], ['ask', '询问']]) {
      const opt = document.createElement('option');
      opt.value = v;
      opt.textContent = label;
      if (v === (e.action || 'auto')) opt.selected = true;
      actionSel.appendChild(opt);
    }
    actionSel.addEventListener('change', () => {
      const list = currentEntries();
      list[idx].action = actionSel.value;
      saveSchedule({ entries: list });
    });

    const toggle = document.createElement('input');
    toggle.type = 'checkbox';
    toggle.className = 'row-toggle';
    toggle.checked = e.enabled !== false;
    toggle.title = '启用此条目';
    toggle.addEventListener('change', () => {
      const list = currentEntries();
      list[idx].enabled = toggle.checked;
      saveSchedule({ entries: list });
    });

    const del = document.createElement('button');
    del.className = 'row-del';
    del.textContent = '✕';
    del.title = '删除此条目';
    del.addEventListener('click', () => {
      const list = currentEntries();
      list.splice(idx, 1);
      saveSchedule({ entries: list });
    });

    row.append(time, modeSel, actionSel, toggle, del);
    scheduleList.appendChild(row);
  });
}

scheduleEnabled.addEventListener('change', () => {
  saveSchedule({ enabled: scheduleEnabled.checked });
});

scheduleAdd.addEventListener('click', () => {
  const list = currentEntries();
  const id = 's' + Date.now().toString(36);
  list.push({ id, time: '12:00', modeId: 'reading', action: 'ask', enabled: true });
  saveSchedule({ entries: list });
});

// ---- 感光监测 ----

function saveAmbient(patch) {
  window.eyeGuard.updateSettings({ ambient: patch }).catch((err) => {
    console.warn('[ui] ambient 保存失败:', err.message);
  });
}

function renderAmbientState(st) {
  if (!st) return;
  if (!st.enabled) {
    ambientStatus.textContent = '未启用';
    return;
  }
  if (st.running) {
    const a = st.analyzer || {};
    const base = typeof a.baseline === 'number' && a.baseline > 0 ? Math.round(a.baseline) : '…';
    const src = st.source === 'als' ? '环境光传感器' : st.source === 'camera' ? '摄像头' : '';
    ambientStatus.textContent = '监测中 · ' + (src ? src + ' · ' : '') + '基线亮度 ' + base + ' · 样本 ' + (a.samples || 0);
  } else if (st.stoppedReason) {
    ambientStatus.textContent = '已停止：' + st.stoppedReason;
  } else {
    ambientStatus.textContent = '已启用（等待采样）';
  }
}

ambientEnabled.addEventListener('change', () => {
  saveAmbient({ enabled: ambientEnabled.checked });
});
ambientInterval.addEventListener('change', () => {
  const v = Math.max(30, Math.min(300, Number(ambientInterval.value) || 60));
  ambientInterval.value = String(v);
  saveAmbient({ intervalSeconds: v });
});
ambientThreshold.addEventListener('change', () => {
  const v = Math.max(10, Math.min(80, Number(ambientThreshold.value) || 35));
  ambientThreshold.value = String(v);
  saveAmbient({ dropThresholdPercent: v });
});
ambientAction.addEventListener('change', () => {
  saveAmbient({ action: ambientAction.value });
});
ambientMode.addEventListener('change', () => {
  saveAmbient({ autoModeId: ambientMode.value });
});

window.eyeGuard.onAmbientState(renderAmbientState);

// ---- 色彩敏感应用（W3）----

// 当前选中的应用（进程名数组，唯一真源）
let colorSensitiveApps = [];
// 扫描到的已安装应用缓存（首次打开时拉取）
let installedAppsCache = null;

function saveColorSensitive(patch) {
  window.eyeGuard.updateSettings({ colorSensitive: patch }).catch((err) => {
    console.warn('[ui] 色彩敏感应用保存失败:', err.message);
  });
}

function renderColorSensitiveStatus() {
  if (!colorSensitiveEnabled.checked) {
    colorSensitiveStatus.textContent = '已关闭';
    return;
  }
  colorSensitiveStatus.textContent = '监控中 · 已选 ' + colorSensitiveApps.length + ' 个应用';
}

/** 渲染已选列表（chips，点击移除） */
function renderChosen() {
  colorSensitiveChosen.textContent = '';
  if (colorSensitiveApps.length === 0) {
    const empty = document.createElement('span');
    empty.className = 'apps-empty';
    empty.textContent = '尚未选择应用——从下方搜索并勾选';
    colorSensitiveChosen.appendChild(empty);
    return;
  }
  for (const proc of colorSensitiveApps) {
    // 已选里的显示名/图标优先用扫描缓存里的；找不到就显示进程名
    const known = installedAppsCache && installedAppsCache.find((a) => a.process === proc);
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'app-chip';
    chip.title = '点击移除 ' + proc;
    if (known && known.icon) {
      const ci = document.createElement('img');
      ci.className = 'app-icon app-icon-chip';
      ci.alt = '';
      ci.src = known.icon;
      chip.appendChild(ci);
    }
    chip.appendChild(document.createTextNode((known ? known.name : proc) + ' ×'));
    chip.addEventListener('click', () => {
      colorSensitiveApps = colorSensitiveApps.filter((p) => p !== proc);
      saveColorSensitive({ apps: colorSensitiveApps });
      renderChosen();
      renderColorSensitiveList();
      renderColorSensitiveStatus();
    });
    colorSensitiveChosen.appendChild(chip);
  }
}

/** 渲染搜索结果列表（勾选） */
function renderColorSensitiveList() {
  if (!installedAppsCache) return;
  const q = colorSensitiveSearch.value.trim().toLowerCase();
  colorSensitiveList.textContent = '';

  // 搜索结果：名称或进程名包含关键词；空查询显示全部（限 60 条防卡顿）
  const filtered = installedAppsCache
    .filter((a) => !q || a.name.toLowerCase().includes(q) || a.process.toLowerCase().includes(q))
    .slice(0, 60);

  if (filtered.length === 0) {
    const none = document.createElement('p');
    none.className = 'apps-empty';
    none.textContent = q ? '没有匹配的应用——可尝试用英文名搜索' : '未扫描到应用';
    colorSensitiveList.appendChild(none);
    return;
  }

  for (const app of filtered) {
    const checked = colorSensitiveApps.includes(app.process);
    const row = document.createElement('label');
    row.className = 'app-row' + (checked ? ' checked' : '');
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = checked;
    box.addEventListener('change', () => {
      if (box.checked) {
        if (!colorSensitiveApps.includes(app.process)) colorSensitiveApps.push(app.process);
      } else {
        colorSensitiveApps = colorSensitiveApps.filter((p) => p !== app.process);
      }
      saveColorSensitive({ apps: colorSensitiveApps });
      renderChosen();
      renderColorSensitiveList();
      renderColorSensitiveStatus();
    });
    const icon = document.createElement('img');
    icon.className = 'app-icon';
    icon.alt = '';
    if (app.icon) {
      icon.src = app.icon;
    } else {
      // 无图标：中性灰块占位（CSS .app-icon-empty），不再用 SVG data URL
      icon.classList.add('app-icon-empty');
    }
    const nameSpan = document.createElement('span');
    nameSpan.className = 'app-name';
    nameSpan.textContent = app.name;
    const procSpan = document.createElement('span');
    procSpan.className = 'app-proc';
    procSpan.textContent = app.process;
    row.append(box, icon, nameSpan, procSpan);
    colorSensitiveList.appendChild(row);
  }
}

colorSensitiveEnabled.addEventListener('change', () => {
  saveColorSensitive({ enabled: colorSensitiveEnabled.checked });
  renderColorSensitiveStatus();
});

colorSensitiveSearch.addEventListener('input', renderColorSensitiveList);
// 懒加载：首次聚焦搜索框时才扫描（避免拖慢启动）
colorSensitiveSearch.addEventListener('focus', ensureInstalledApps, { once: true });

/** 首次展开时拉取已安装应用（懒加载；失败静默降级为已选列表手动输入态） */
async function ensureInstalledApps() {
  if (installedAppsCache) return;
  colorSensitiveList.textContent = '正在扫描已安装应用…';
  try {
    const apps = await window.eyeGuard.listInstalledApps();
    installedAppsCache = Array.isArray(apps) ? apps : [];
  } catch (err) {
    console.warn('[ui] 应用扫描失败:', err.message);
    installedAppsCache = [];
  }
  renderColorSensitiveList();
  renderChosen();
}

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
  const [versionR, settingsR, modesR, stateR, autoR, breakR, ambientR] = await Promise.allSettled([
    window.eyeGuard.getVersion(),
    window.eyeGuard.getSettings(),
    window.eyeGuard.listModes(),
    window.eyeGuard.getDisplayState(),
    window.eyeGuard.getAutoLaunch(),
    window.eyeGuard.getBreakState(),
    window.eyeGuard.getAmbientState()
  ]);

  const settings = settingsR.status === 'fulfilled' ? settingsR.value : null;
  settingsCache = settings;
  // 主题：白名单外的旧值（如旧版 "deepsea"）归一化为默认深色，并写回设置
  let theme = settings && settings.theme;
  if (!THEMES.includes(theme)) {
    theme = 'dark';
    if (settings) window.eyeGuard.updateSettings({ theme }).catch(() => {});
  }
  applyTheme(theme);

  if (versionR.status === 'fulfilled') versionEl.textContent = 'v' + versionR.value;

  if (modesR.status === 'fulfilled' && Array.isArray(modesR.value)) {
    modesCache = modesR.value;
  }
  if (settings && settings.modeId) currentModeId = settings.modeId;
  renderModes();

  // 感光目标模式下拉（排除原色——"变暗切到原色"无意义）
  ambientMode.innerHTML = '';
  for (const m of modesCache) {
    if (m.id === 'natural') continue;
    const opt = document.createElement('option');
    opt.value = m.id;
    opt.textContent = m.name + '（' + m.kelvin + 'K）';
    ambientMode.appendChild(opt);
  }
  if (settings && settings.ambient) {
    ambientEnabled.checked = !!settings.ambient.enabled;
    ambientInterval.value = String(settings.ambient.intervalSeconds || 60);
    ambientThreshold.value = String(settings.ambient.dropThresholdPercent || 35);
    ambientAction.value = settings.ambient.action || 'notify';
    if (settings.ambient.autoModeId) ambientMode.value = settings.ambient.autoModeId;
  }
  if (settings && settings.colorSensitive) {
    colorSensitiveEnabled.checked = !!settings.colorSensitive.enabled;
    colorSensitiveApps = Array.isArray(settings.colorSensitive.apps)
      ? settings.colorSensitive.apps.slice()
      : [];
    renderChosen();
    renderColorSensitiveStatus();
    renderColorSensitiveList();
  }
  renderSchedule();

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

  if (ambientR.status === 'fulfilled') renderAmbientState(ambientR.value);

  document.activeElement && document.activeElement.blur();
}

window.addEventListener('DOMContentLoaded', boot);

// ---- 板块导航（分板块切换：显示调节 / 休息提醒 / 自动化 / 通用）----

function switchPanel(name) {
  document.querySelectorAll('.panel').forEach((p) => {
    p.classList.toggle('active', p.dataset.panel === name);
  });
  document.querySelectorAll('.nav-item').forEach((b) => {
    b.classList.toggle('active', b.dataset.panelTarget === name);
  });
}

document.querySelectorAll('.nav-item').forEach((btn) => {
  btn.addEventListener('click', () => switchPanel(btn.dataset.panelTarget));
});

// ---- 无边框窗口的自绘按钮 ----

$('win-min').addEventListener('click', () => {
  window.eyeGuard.windowMinimize().catch(() => {});
});

$('win-close').addEventListener('click', () => {
  window.eyeGuard.windowHide().catch(() => {});
});

// ---- 自绘下拉弹层（替代原生 select 弹层，风格与整体统一）----
// 策略：保留原生 <select>（表单语义 / 既有 change 监听 / 键盘无障碍不变），
// 鼠标 mousedown 时拦截原生弹层、渲染自绘弹层；选择后写回 value 并派发 change。

let selectPopEl = null;

function closeSelectPop() {
  if (selectPopEl) {
    selectPopEl.remove();
    selectPopEl = null;
  }
}

document.addEventListener('mousedown', (e) => {
  const sel = e.target && e.target.closest ? e.target.closest('select') : null;
  if (!sel) {
    closeSelectPop();
    return;
  }
  e.preventDefault(); // 阻止原生弹层
  closeSelectPop();

  const pop = document.createElement('div');
  pop.className = 'select-pop';
  Array.from(sel.options).forEach((opt, i) => {
    const row = document.createElement('div');
    row.className = 'opt' + (i === sel.selectedIndex ? ' sel' : '');
    row.textContent = opt.textContent;
    row.addEventListener('mousedown', (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      sel.value = opt.value;
      sel.dispatchEvent(new Event('change', { bubbles: true }));
      closeSelectPop();
    });
    pop.appendChild(row);
  });
  document.body.appendChild(pop);

  const rect = sel.getBoundingClientRect();
  pop.style.minWidth = Math.max(rect.width, 120) + 'px';
  pop.style.left = rect.left + 'px';
  const below = rect.bottom + 4;
  if (below + pop.offsetHeight > window.innerHeight - 8) {
    pop.style.top = Math.max(8, rect.top - pop.offsetHeight - 4) + 'px';
  } else {
    pop.style.top = below + 'px';
  }
  selectPopEl = pop;
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeSelectPop();
});

// 滚动 / 切板块时关闭，避免错位残留
document.addEventListener('wheel', closeSelectPop, { passive: true });
document.querySelectorAll('.nav-item').forEach((btn) => {
  btn.addEventListener('click', closeSelectPop);
});

// ---- 日落跟随（自动化板块）----

// 预设城市坐标（lat, lon）——用户选城市一次，之后完全离线
const SUN_CITIES = [
  { name: '北京', lat: 39.90, lon: 116.41 },
  { name: '上海', lat: 31.23, lon: 121.47 },
  { name: '广州', lat: 23.13, lon: 113.26 },
  { name: '深圳', lat: 22.54, lon: 114.06 },
  { name: '成都', lat: 30.57, lon: 104.07 },
  { name: '杭州', lat: 30.27, lon: 120.16 },
  { name: '武汉', lat: 30.59, lon: 114.31 },
  { name: '西安', lat: 34.34, lon: 108.94 },
  { name: '沈阳', lat: 41.80, lon: 123.43 },
  { name: '乌鲁木齐', lat: 43.83, lon: 87.62 }
];

function fmtMinutes(m) {
  if (typeof m !== 'number' || !isFinite(m)) return '--:--';
  let t = Math.round(m);
  t = ((t % 1440) + 1440) % 1440;
  const h = Math.floor(t / 60);
  const mm = String(t % 60).padStart(2, '0');
  return String(h).padStart(2, '0') + ':' + mm;
}

function renderSunNote(sun) {
  const el = $('sun-note');
  if (!el) return;
  if (!sun || typeof sun.lat !== 'number') {
    el.textContent = '未设置位置——点「自动定位」或选择城市';
    return;
  }
  window.eyeGuard
    .sunTimesToday()
    .then((t) => {
      if (!t || !t.ok || typeof t.sunset !== 'number') {
        el.textContent = '日落时间暂不可用';
        return;
      }
      const label = sun.cityLabel ? sun.cityLabel + ' · ' : '';
      // 过渡时长固定 60 分钟（f.lux 同量级；步长远低于可觉差，已由单测断言）
      el.textContent =
        label + '今日日落 ' + fmtMinutes(t.sunset) + ' · ' + fmtMinutes(t.sunset - 60) + ' 开始过渡';
    })
    .catch(() => {
      el.textContent = '日落时间暂不可用';
    });
}

function bindSunFollow(settings) {
  const sun = (settings && settings.sunFollow) || {};
  const enabled = $('sun-enabled');
  const city = $('sun-city');
  const target = $('sun-target');
  const locateBtn = $('sun-locate');
  if (!enabled || !city || !target) return;

  const fillCities = () => {
    city.innerHTML = '';
    // 首项空值：未主动选择时不落库（防"启用即隐式采用默认城市"）
    const placeholder = document.createElement('option');
    placeholder.value = '';
    placeholder.textContent = '请选择城市…';
    city.appendChild(placeholder);
    SUN_CITIES.forEach((c, i) => {
      const opt = document.createElement('option');
      opt.value = String(i);
      opt.textContent = c.name;
      city.appendChild(opt);
    });
  };
  fillCities();

  // 坐标 → 选中项（命中预设城市则选中；否则复用/更新单个"自动定位"项——避免重复累积）
  const applyCoords = (lat, lon, label) => {
    const idx = SUN_CITIES.findIndex(
      (c) => Math.abs(c.lat - lat) < 0.05 && Math.abs(c.lon - lon) < 0.05
    );
    if (idx >= 0) {
      city.value = String(idx);
      return;
    }
    let opt = city.querySelector('option[value="auto"]');
    if (!opt) {
      opt = document.createElement('option');
      opt.value = 'auto';
      city.appendChild(opt);
    }
    opt.textContent = label || '自动定位';
    opt.dataset.lat = String(lat);
    opt.dataset.lon = String(lon);
    city.value = 'auto';
  };

  // 未选择时返回 null（不落库）
  const readCoords = () => {
    if (city.value === '') return null;
    if (city.value === 'auto') {
      const opt = city.selectedOptions[0];
      if (!opt || opt.dataset.lat === undefined) return null;
      return {
        lat: Number(opt.dataset.lat),
        lon: Number(opt.dataset.lon),
        label: opt.textContent || ''
      };
    }
    const c = SUN_CITIES[Number(city.value)];
    return c ? { lat: c.lat, lon: c.lon, label: c.name } : null;
  };

  if (typeof sun.lat === 'number' && typeof sun.lon === 'number') {
    applyCoords(sun.lat, sun.lon, sun.cityLabel);
  }
  enabled.checked = !!sun.enabled;
  if (sun.targetModeId) target.value = sun.targetModeId;

  const commit = () => {
    const coords = readCoords();
    const patch = {
      sunFollow: {
        enabled: enabled.checked,
        // 未选城市：坐标保持为空（不隐式落库）
        lat: coords ? coords.lat : null,
        lon: coords ? coords.lon : null,
        cityLabel: coords ? coords.label : sun.cityLabel || null,
        targetModeId: target.value
        // 过渡窗口固定 60 分钟（后端 sun_rt 常量为权威，不落盘该字段）
      }
    };
    window.eyeGuard
      .updateSettings(patch)
      .then(() => {
        renderSunNote(patch.sunFollow);
        // 开启但尚无坐标 → 立即尝试自动定位（运行时启用同样触发）
        if (enabled.checked && !coords) {
          doLocate();
        }
      })
      .catch(() => {
        const el = $('sun-note');
        if (el) el.textContent = '设置保存失败，请重试';
      });
  };

  // 自动定位：城市名（国内库，准）+ 坐标；失败提示手动选
  const doLocate = () => {
    if (locateBtn) locateBtn.disabled = true;
    window.eyeGuard
      .sunAutolocate()
      .then((r) => {
        if (r && r.ok && typeof r.lat === 'number' && typeof r.lon === 'number') {
          applyCoords(r.lat, r.lon, r.city);
          commit();
        } else {
          const el = $('sun-note');
          if (el) el.textContent = '自动定位失败——请手动选择城市';
        }
      })
      .catch(() => {
        const el = $('sun-note');
        if (el) el.textContent = '自动定位失败——请手动选择城市';
      })
      .finally(() => {
        if (locateBtn) locateBtn.disabled = false;
      });
  };

  [enabled, city, target].forEach((el) => el.addEventListener('change', commit));
  if (locateBtn) locateBtn.addEventListener('click', doLocate);

  renderSunNote(sun);

  // 已启用但从未定位过 → 启动即尝试一次自动定位（成功/失败都会给出明确提示）
  if (sun.enabled && typeof sun.lat !== 'number') {
    doLocate();
  }
}

window.addEventListener('DOMContentLoaded', () => {
  window.eyeGuard
    .getSettings()
    .then((s) => bindSunFollow(s))
    .catch(() => {});
});
