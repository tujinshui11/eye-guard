'use strict';

const { contextBridge, ipcRenderer } = require('electron');

// 白名单 IPC API
contextBridge.exposeInMainWorld('eyeGuard', {
  getVersion: () => ipcRenderer.invoke('app:get-version'),
  // 色温 / 亮度
  getDisplayState: () => ipcRenderer.invoke('display:get-state'),
  setTemperature: (k) => ipcRenderer.invoke('display:set-temperature', k),
  setBrightness: (b) => ipcRenderer.invoke('display:set-brightness', b),
  restoreColor: () => ipcRenderer.invoke('display:restore'),
  // 设置
  getSettings: () => ipcRenderer.invoke('settings:get'),
  updateSettings: (patch) => ipcRenderer.invoke('settings:set', patch),
  // 开机自启
  setAutoLaunch: (enabled) => ipcRenderer.invoke('app:set-auto-launch', enabled),
  getAutoLaunch: () => ipcRenderer.invoke('app:get-auto-launch'),
  // 休息提醒
  getBreakState: () => ipcRenderer.invoke('break:get-state'),
  breakAction: (name) => ipcRenderer.invoke('break:action', name),
  onBreakUpdate: (cb) => {
    ipcRenderer.on('break:update', (e, state) => cb(state));
  }
});
