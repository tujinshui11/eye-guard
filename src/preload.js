'use strict';

const { contextBridge, ipcRenderer } = require('electron');

// 白名单 IPC API（W1 基础版；W2 起扩展色温/亮度/提醒通道）
contextBridge.exposeInMainWorld('eyeGuard', {
  getVersion: () => ipcRenderer.invoke('app:get-version')
});
