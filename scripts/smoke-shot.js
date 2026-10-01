'use strict';

// 开发辅助脚本：无头启动设置面板并截图（跳过托盘/单实例逻辑，仅验证渲染与 preload 链路）
// 用法：npx electron scripts/smoke-shot.js [输出路径.png] [主题 deepsea|warm|anime] [scroll: top|bottom]
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const fs = require('fs');

const out = process.argv[2] || path.join(__dirname, '..', '.rivet', 'scratch', 'smoke', 'ui.png');
const THEMES = ['deepsea', 'warm', 'anime'];
const theme = THEMES.includes(process.argv[3]) ? process.argv[3] : 'deepsea';
const scroll = process.argv[4] === 'bottom' ? 'bottom' : 'top';

const { MODES } = require('../src/main/modes');

ipcMain.handle('app:get-version', () => app.getVersion());
// 与产品同形的只读 IPC（供 UI 回填；无副作用）
ipcMain.handle('display:get-state', () => ({
  available: true,
  enabled: true,
  temperature: 5500,
  brightness: 100,
  modeId: 'office'
}));
ipcMain.handle('modes:list', () => MODES.map((m) => Object.assign({}, m)));
ipcMain.handle('settings:get', () => ({
  enabled: true,
  temperature: 5500,
  brightness: 100,
  modeId: 'office',
  theme,
  breaks: { enabled: true, workSeconds: 1200, breakSeconds: 20, style: 'gentle' },
  hotkeys: { tempUp: 'Control+Alt+Up', tempDown: 'Control+Alt+Down' },
  autoLaunch: false,
  pausedUntil: null,
  schedule: {
    enabled: true,
    entries: [
      { id: 's1', time: '09:00', modeId: 'office', action: 'auto', enabled: true },
      { id: 's2', time: '18:00', modeId: 'evening', action: 'ask', enabled: true },
      { id: 's3', time: '22:00', modeId: 'night', action: 'auto', enabled: false }
    ],
    lastFired: {}
  },
  ambient: {
    enabled: true,
    intervalSeconds: 60,
    dropThresholdPercent: 35,
    action: 'notify',
    autoModeId: 'night',
    cooldownMinutes: 15
  }
}));
ipcMain.handle('app:get-auto-launch', () => ({ openAtLogin: false, args: [] }));
ipcMain.handle('break:get-state', () => ({ state: 'working', remainingSeconds: 754, pausedUntil: null }));
ipcMain.handle('ambient:get-state', () => ({
  enabled: true,
  running: true,
  stoppedReason: null,
  failures: 0,
  analyzer: { smooth: 92.4, baseline: 95.1, state: 'normal', samples: 12 }
}));

app.whenReady().then(async () => {
  const win = new BrowserWindow({
    width: 440,
    height: 680,
    show: false,
    backgroundColor: '#0B0F1A',
    webPreferences: {
      preload: path.join(__dirname, '..', 'src', 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    }
  });
  await win.loadFile(path.join(__dirname, '..', 'src', 'renderer', 'index.html'));
  await new Promise((r) => setTimeout(r, 1200)); // 等 DOMContentLoaded 后的 IPC 回填
  if (scroll === 'bottom') {
    await win.webContents.executeJavaScript(
      "(() => { const m = document.querySelector('main'); if (m) m.scrollTop = m.scrollHeight; return m ? m.scrollTop : -1; })()"
    );
    await new Promise((r) => setTimeout(r, 400));
  }
  const image = await win.webContents.capturePage();
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, image.toPNG());
  console.log('[smoke] screenshot saved:', out, '(theme=' + theme + ')');
  app.exit(0);
});
