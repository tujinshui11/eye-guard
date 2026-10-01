'use strict';

const { contextBridge, ipcRenderer } = require('electron');

// 白名单 IPC API
contextBridge.exposeInMainWorld('eyeGuard', {
  getVersion: () => ipcRenderer.invoke('app:get-version'),
  // 色温（W2）
  getDisplayState: () => ipcRenderer.invoke('display:get-state'),
  setTemperature: (k) => ipcRenderer.invoke('display:set-temperature', k),
  restoreColor: () => ipcRenderer.invoke('display:restore')
});
