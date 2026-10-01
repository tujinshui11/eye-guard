'use strict';

// 设置面板交互（W4：色温 + 亮度 + 休息提醒）

const tempSlider = document.getElementById('temperature');
const tempValue = document.getElementById('temperature-value');
const tempNote = document.getElementById('temp-note');
const brightSlider = document.getElementById('brightness');
const brightValue = document.getElementById('brightness-value');
const versionEl = document.getElementById('version');

const breakEnabled = document.getElementById('break-enabled');
const breakWork = document.getElementById('break-work');
const breakRest = document.getElementById('break-rest');
const breakStyle = document.getElementById('break-style');
const breakNote = document.getElementById('break-note');

// ---- 通用 ----

function throttled(fn, ms) {
  let timer = null;
  let pending = null;
  const flush = () => {
    timer = null;
    if (pending !== null) {
      const v = pending;
      pending = null;
      fn(v);
    }
  };
  return {
    input(v) {
      pending = v;
      if (!timer) timer = setTimeout(flush, ms);
    },
    commit(v) {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      pending = null;
      fn(v);
    }
  };
}

function showNote(el, text) {
  el.textContent = text;
  el.classList.toggle('hidden', !text);
}

// ---- 色温 ----

async function applyTemperature(k) {
  try {
    const res = await window.eyeGuard.setTemperature(k);
    if (res && res.ok) {
      tempValue.textContent = res.effectiveTemperature;
      showNote(tempNote, res.clamped ? '受显卡限制，本机实际最低约 ' + res.effectiveTemperature + 'K' : '');
    } else {
      showNote(tempNote, (res && res.error) || '应用失败');
    }
  } catch (err) {
    showNote(tempNote, 'IPC 调用失败：' + err.message);
  }
}

const tempThrottle = throttled(applyTemperature, 40);

tempSlider.addEventListener('input', () => {
  tempValue.textContent = tempSlider.value;
  tempThrottle.input(Number(tempSlider.value));
});
tempSlider.addEventListener('change', () => tempThrottle.commit(Number(tempSlider.value)));

document.querySelectorAll('.presets button[data-k]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const k = Number(btn.dataset.k);
    tempSlider.value = String(k);
    tempValue.textContent = String(k);
    applyTemperature(k);
  });
});

// ---- 亮度 ----

const brightThrottle = throttled((b) => {
  window.eyeGuard.setBrightness(b).catch(() => {});
}, 40);

brightSlider.addEventListener('input', () => {
  brightValue.textContent = brightSlider.value;
  brightThrottle.input(Number(brightSlider.value));
});
brightSlider.addEventListener('change', () => brightThrottle.commit(Number(brightSlider.value)));

// ---- 休息提醒 ----

function collectBreaks() {
  return {
    enabled: breakEnabled.checked,
    workSeconds: Math.max(60, Math.round(Number(breakWork.value || 40) * 60)),
    breakSeconds: Math.max(5, Math.round(Number(breakRest.value || 300))),
    style: breakStyle.value
  };
}

function pushBreaks() {
  window.eyeGuard.updateSettings({ breaks: collectBreaks() }).catch(() => {});
}

breakEnabled.addEventListener('change', pushBreaks);
breakWork.addEventListener('change', pushBreaks);
breakRest.addEventListener('change', pushBreaks);
breakStyle.addEventListener('change', pushBreaks);

document.getElementById('preset-2020').addEventListener('click', () => {
  breakWork.value = '20';
  breakRest.value = '20';
  pushBreaks();
  showNote(breakNote, '已应用 20-20-20 规则：每 20 分钟，看 20 英尺外 20 秒');
});

document.getElementById('break-pause').addEventListener('click', async () => {
  try {
    await window.eyeGuard.breakAction('pause1h');
    showNote(breakNote, '已暂停提醒 1 小时（托盘菜单可恢复）');
  } catch (err) {
    showNote(breakNote, '暂停失败：' + err.message);
  }
});

// ---- 启动状态 ----

window.addEventListener('DOMContentLoaded', async () => {
  try {
    const version = await window.eyeGuard.getVersion();
    versionEl.textContent = 'v' + version;
  } catch (err) {
    console.warn('版本读取失败:', err && err.message);
  }

  try {
    const state = await window.eyeGuard.getDisplayState();
    if (state) {
      if (state.temperature) {
        tempSlider.value = String(state.temperature);
        tempValue.textContent = String(state.temperature);
      }
      if (state.brightness) {
        brightSlider.value = String(state.brightness);
        brightValue.textContent = String(state.brightness);
      }
    }
  } catch (err) {
    console.warn('显示状态读取失败:', err && err.message);
  }

  try {
    const settings = await window.eyeGuard.getSettings();
    if (settings && settings.breaks) {
      breakEnabled.checked = !!settings.breaks.enabled;
      breakWork.value = String(Math.round(settings.breaks.workSeconds / 60));
      breakRest.value = String(settings.breaks.breakSeconds);
      breakStyle.value = settings.breaks.style || 'gentle';
    }
  } catch (err) {
    console.warn('设置读取失败:', err && err.message);
  }
});
