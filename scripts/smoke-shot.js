'use strict';

// 开发辅助脚本：无头启动设置面板并截图（跳过托盘/单实例逻辑，仅验证渲染与 preload 链路）
// 用法：npx electron scripts/smoke-shot.js [输出路径.png]
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const fs = require('fs');

const out = process.argv[2] || path.join(__dirname, '..', '.rivet', 'scratch', 'smoke', 'ui.png');

ipcMain.handle('app:get-version', () => app.getVersion());
// 与产品同形的只读 IPC（供 UI 回填；无副作用）
ipcMain.handle('display:get-state', () => ({
  available: true,
  enabled: true,
  temperature: 5500,
  brightness: 100
}));
ipcMain.handle('settings:get', () => ({
  breaks: { enabled: true, workSeconds: 1200, breakSeconds: 20, style: 'gentle' }
}));
ipcMain.handle('app:get-auto-launch', () => ({ openAtLogin: false, args: [] }));

app.whenReady().then(async () => {
  const win = new BrowserWindow({
    width: 420,
    height: 560,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, '..', 'src', 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    }
  });
  await win.loadFile(path.join(__dirname, '..', 'src', 'renderer', 'index.html'));
  await new Promise((r) => setTimeout(r, 800)); // 等 DOMContentLoaded 后的 IPC 回填
  const image = await win.webContents.capturePage();
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, image.toPNG());
  console.log('[smoke] screenshot saved:', out);
  app.exit(0);
});
