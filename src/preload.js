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
  // 场景模式（v2）
  listModes: () => ipcRenderer.invoke('modes:list'),
  applyMode: (modeId) => ipcRenderer.invoke('modes:apply', modeId),
  // 设置
  getSettings: () => ipcRenderer.invoke('settings:get'),
  updateSettings: (patch) => ipcRenderer.invoke('settings:set', patch),
  // 开机自启
  setAutoLaunch: (enabled) => ipcRenderer.invoke('app:set-auto-launch', enabled),
  getAutoLaunch: () => ipcRenderer.invoke('app:get-auto-launch'),
  // 休息提醒
  getBreakState: () => ipcRenderer.invoke('break:get-state'),
  breakAction: (name) => ipcRenderer.invoke('break:action', name),
  // 建议卡片（v2：时间调度询问 / 感光提醒共用）
  proposeAction: (name) => ipcRenderer.invoke('propose:action', name),
  onPropose: (cb) => {
    ipcRenderer.on('propose:update', (e, payload) => cb(payload));
  },
  // 感光监测状态（v2）
  getAmbientState: () => ipcRenderer.invoke('ambient:get-state'),
  onAmbientState: (cb) => {
    ipcRenderer.on('ambient:state', (e, st) => cb(st));
  },
  // 主进程 → 渲染进程广播
  onBreakUpdate: (cb) => {
    ipcRenderer.on('break:update', (e, state) => cb(state));
  },
  onDisplayChanged: (cb) => {
    ipcRenderer.on('display:changed', (e, state) => cb(state));
  },
  onThemeChanged: (cb) => {
    ipcRenderer.on('theme:changed', (e, theme) => cb(theme));
  }
});
