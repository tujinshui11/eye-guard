'use strict';

// 设置面板交互（W3：色温 + 亮度接线；W4+ 扩展提醒/通用）

const tempSlider = document.getElementById('temperature');
const tempValue = document.getElementById('temperature-value');
const tempNote = document.getElementById('temp-note');
const brightSlider = document.getElementById('brightness');
const brightValue = document.getElementById('brightness-value');
const versionEl = document.getElementById('version');

// ---- 通用节流 ----
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

function showNote(text) {
  tempNote.textContent = text;
  tempNote.classList.toggle('hidden', !text);
}

// ---- 色温 ----

async function applyTemperature(k) {
  try {
    const res = await window.eyeGuard.setTemperature(k);
    if (res && res.ok) {
      tempValue.textContent = res.effectiveTemperature;
      if (res.clamped) {
        showNote('受显卡限制，本机实际最低约 ' + res.effectiveTemperature + 'K');
      } else {
        showNote('');
      }
    } else {
      showNote((res && res.error) || '应用失败');
    }
  } catch (err) {
    showNote('IPC 调用失败：' + err.message);
  }
}

const tempThrottle = throttled(applyTemperature, 40);

tempSlider.addEventListener('input', () => {
  tempValue.textContent = tempSlider.value;
  tempThrottle.input(Number(tempSlider.value));
});

tempSlider.addEventListener('change', () => {
  tempThrottle.commit(Number(tempSlider.value));
});

document.querySelectorAll('.presets button').forEach((btn) => {
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

brightSlider.addEventListener('change', () => {
  brightThrottle.commit(Number(brightSlider.value));
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
});
