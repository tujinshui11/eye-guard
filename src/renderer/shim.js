// eyeGuard shim —— 把 Electron preload 暴露的 API 原样映射到 Tauri。
// 契约来源：src/preload.js（15 invoke + 5 事件订阅）；事件名与参数名保持一致。
(function () {
  'use strict';
  const tauri = window.__TAURI__;
  if (!tauri || !tauri.core || !tauri.event) {
    console.error('[shim] Tauri 全局 API 不可用（withGlobalTauri 未开启？）');
    return;
  }
  const invoke = tauri.core.invoke;
  const listen = tauri.event.listen;

  window.eyeGuard = {
    // ---- invoke 类（15）—— 参数名与 Rust command 的 snake_case 自动互转 ----
    getVersion: () => invoke('app_get_version'),
    getDisplayState: () => invoke('display_get_state'),
    setTemperature: (kelvin) => invoke('display_set_temperature', { kelvin }),
    setBrightness: (brightness) => invoke('display_set_brightness', { brightness }),
    restoreColor: () => invoke('display_restore'),
    listModes: () => invoke('modes_list'),
    applyMode: (modeId) => invoke('modes_apply', { modeId }),
    getSettings: () => invoke('settings_get'),
    updateSettings: (patch) => invoke('settings_set', { patch }),
    setAutoLaunch: (enabled) => invoke('app_set_auto_launch', { enabled }),
    getAutoLaunch: () => invoke('app_get_auto_launch'),
    getBreakState: () => invoke('break_get_state'),
    breakAction: (name) => invoke('break_action', { name }),
    proposeAction: (name) => invoke('propose_action', { name }),
    getAmbientState: () => invoke('ambient_get_state'),

    // ---- 事件订阅（5）—— 事件名与 Electron 版一致 ----
    onPropose: (cb) => { listen('propose:update', (e) => cb(e.payload)); },
    onAmbientState: (cb) => { listen('ambient:state', (e) => cb(e.payload)); },
    onBreakUpdate: (cb) => { listen('break:update', (e) => cb(e.payload)); },
    onDisplayChanged: (cb) => { listen('display:changed', (e) => cb(e.payload)); },
    onThemeChanged: (cb) => { listen('theme:changed', (e) => cb(e.payload)); }
  };
})();
