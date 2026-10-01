'use strict';

// 设置面板交互（W2：色温接线；W3+ 扩展亮度/提醒/通用）

const tempSlider = document.getElementById('temperature');
const tempValue = document.getElementById('temperature-value');
const tempNote = document.getElementById('temp-note');
const versionEl = document.getElementById('version');

let throttleTimer = null;
let pendingK = null;

function showNote(text) {
  tempNote.textContent = text;
  tempNote.classList.toggle('hidden', !text);
}

async function applyTemperature(k) {
  try {
    const res = await window.eyeGuard.setTemperature(k);
    if (res && res.ok) {
      // 显示实际生效色温（可能因显卡限制被钳制）
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

function scheduleApply(k) {
  pendingK = k;
  if (throttleTimer) return;
  throttleTimer = setTimeout(() => {
    throttleTimer = null;
    if (pendingK !== null) {
      const k2 = pendingK;
      pendingK = null;
      applyTemperature(k2);
    }
  }, 40);
}

tempSlider.addEventListener('input', () => {
  tempValue.textContent = tempSlider.value;
  scheduleApply(Number(tempSlider.value));
});

// 松开滑块：立即应用最终值（覆盖节流尾巴）
tempSlider.addEventListener('change', () => {
  if (throttleTimer) {
    clearTimeout(throttleTimer);
    throttleTimer = null;
  }
  pendingK = null;
  applyTemperature(Number(tempSlider.value));
});

document.querySelectorAll('.presets button').forEach((btn) => {
  btn.addEventListener('click', () => {
    const k = Number(btn.dataset.k);
    tempSlider.value = String(k);
    tempValue.textContent = String(k);
    applyTemperature(k);
  });
});

window.addEventListener('DOMContentLoaded', async () => {
  try {
    const version = await window.eyeGuard.getVersion();
    versionEl.textContent = 'v' + version;
  } catch (err) {
    console.warn('版本读取失败:', err && err.message);
  }
  try {
    const state = await window.eyeGuard.getDisplayState();
    if (state && state.available && state.temperature) {
      tempSlider.value = String(state.temperature);
      tempValue.textContent = String(state.temperature);
    }
  } catch (err) {
    console.warn('显示状态读取失败:', err && err.message);
  }
});
