'use strict';

const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { DisplayController } = require('./display');

// ---- 单实例锁：护眼工具必须常驻唯一实例 ----
const gotTheLock = app.requestSingleInstanceLock();
if (!gotTheLock) {
  app.quit();
} else {
  const isHiddenLaunch = process.argv.includes('--hidden');
  let mainWindow = null;
  let tray = null;
  let display = null;
  let quitting = false;

  function createMainWindow() {
    mainWindow = new BrowserWindow({
      width: 420,
      height: 560,
      show: false,
      resizable: false,
      maximizable: false,
      fullscreenable: false,
      title: '护眼助手',
      icon: path.join(__dirname, '..', '..', 'assets', 'icon.png'),
      webPreferences: {
        preload: path.join(__dirname, '..', 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true
      }
    });
    mainWindow.setMenuBarVisibility(false);
    mainWindow.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'));

    // 关闭 = 隐藏到托盘（不退出）
    mainWindow.on('close', (e) => {
      if (!quitting) {
        e.preventDefault();
        mainWindow.hide();
      }
    });

    mainWindow.once('ready-to-show', () => {
      if (!isHiddenLaunch) mainWindow.show();
      console.log('[boot] window ready, hiddenLaunch=' + isHiddenLaunch);
    });
  }

  function showWindow() {
    if (!mainWindow) return;
    mainWindow.show();
    mainWindow.focus();
  }

  function toggleWindow() {
    if (!mainWindow) return;
    if (mainWindow.isVisible() && mainWindow.isFocused()) {
      mainWindow.hide();
    } else {
      showWindow();
    }
  }

  function quitApp() {
    quitting = true;
    app.quit();
  }

  ipcMain.handle('app:get-version', () => app.getVersion());

  // ---- 显示控制器（gamma ramp 色温调节）----
  function initDisplay() {
    try {
      display = new DisplayController({ dataDir: app.getPath('userData') });
      const { healed } = display.init();
      console.log('[boot] display initialized, healed=' + healed);
    } catch (err) {
      display = null;
      console.error('[boot] display init failed:', err.message);
    }
  }

  ipcMain.handle('display:set-temperature', (e, k) => {
    if (!display) return { ok: false, error: '当前环境不支持屏幕色温调节' };
    const t = Math.min(6500, Math.max(2000, Number(k) || 6500));
    try {
      return display.apply(t);
    } catch (err) {
      return { ok: false, error: err.message };
    }
  });

  ipcMain.handle('display:get-state', () => ({
    available: !!display,
    enabled: display ? display.current.enabled : false,
    temperature: display ? display.current.temperature : 6500
  }));

  ipcMain.handle('display:restore', () => {
    if (!display) return false;
    try {
      return display.restore();
    } catch (err) {
      console.error('[ipc] display:restore 失败:', err.message);
      return false;
    }
  });

  app.whenReady().then(() => {
    console.log('[boot] app ready');
    initDisplay();
    createMainWindow();

    const { createTray } = require('./tray');
    tray = createTray({ onToggleWindow: toggleWindow, onQuit: quitApp });
    console.log('[boot] tray created');

    console.log('[boot] all initialized');
  });

  app.on('second-instance', () => {
    console.log('[boot] second instance detected, showing window');
    showWindow();
  });

  // 托盘常驻：窗口全关不退出（退出走托盘菜单 / app.quit）
  app.on('window-all-closed', () => {
    // no-op
  });

  app.on('before-quit', () => {
    quitting = true;
    // 退出前恢复屏幕原始色彩（dirty 兜底见 display.js）
    if (display && display.originalRamp) {
      try {
        const ok = display.restore();
        console.log('[quit] 恢复原始色彩: ' + (ok ? 'OK' : 'FAILED'));
      } catch (err) {
        console.warn('[quit] 恢复失败:', err.message);
      }
    }
  });
}
