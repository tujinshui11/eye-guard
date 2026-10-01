'use strict';

const { app, BrowserWindow, ipcMain, screen } = require('electron');
const path = require('path');
const { DisplayController } = require('./display');
const { SettingsStore } = require('./settings');
const { BrightnessOverlay } = require('./overlay');
const { registerHotkeys, unregisterAll } = require('./hotkeys');

// ---- 单实例锁：护眼工具必须常驻唯一实例 ----
const gotTheLock = app.requestSingleInstanceLock();
if (!gotTheLock) {
  app.quit();
} else {
  const isHiddenLaunch = process.argv.includes('--hidden');
  let mainWindow = null;
  let tray = null;
  let display = null;
  let overlay = null;
  let settings = null;
  let quitting = false;

  // ==== 窗口 ====

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

  // ==== 业务 ====

  function initSettings() {
    settings = new SettingsStore({ dataDir: app.getPath('userData') });
    const data = settings.load();
    console.log(
      '[boot] settings loaded: temperature=' + data.temperature + ' brightness=' + data.brightness
    );
  }

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

  /** 应用色温（可选持久化） */
  function setTemperature(k, { persist = true, preset = 'custom' } = {}) {
    if (!display) return { ok: false, error: '当前环境不支持屏幕色温调节' };
    const t = Math.min(6500, Math.max(2000, Number(k) || 6500));
    let res;
    try {
      res = display.apply(t);
    } catch (err) {
      return { ok: false, error: err.message };
    }
    if (res.ok && persist && settings) {
      settings.save({ temperature: res.effectiveTemperature, preset });
    }
    return res;
  }

  /** 轻量状态：热键/托盘使用 */
  function nudgeTemperature(delta) {
    const cur = display ? display.current.temperature : 6500;
    const next = Math.min(6500, Math.max(2000, cur + delta));
    console.log('[temp] nudge ' + delta + ' -> ' + next);
    return setTemperature(next);
  }

  function setBrightness(b, { persist = true } = {}) {
    if (!overlay) return { ok: false, brightness: 100 };
    const v = overlay.setBrightness(b);
    if (persist && settings) settings.save({ brightness: v });
    return { ok: true, brightness: v };
  }

  function restoreColor() {
    if (settings) settings.save({ enabled: false });
    if (!display) return false;
    try {
      return display.restore();
    } catch (err) {
      console.error('[display] restore 失败:', err.message);
      return false;
    }
  }

  function reenableColor() {
    if (settings) settings.save({ enabled: true });
    const t = settings ? settings.get().temperature : 4500;
    return setTemperature(t, { persist: true, preset: settings ? settings.get().preset : 'custom' });
  }

  /** 供托盘使用的聚合操作 */
  const trayActions = {
    applyPreset: (k, name) => {
      if (settings) settings.save({ enabled: true });
      return setTemperature(k, { persist: true, preset: name });
    },
    tempDelta: nudgeTemperature,
    restoreColor,
    reenableColor,
    getState: () => ({
      temperature: display ? display.current.temperature : 6500,
      enabled: settings ? settings.get().enabled : true
    })
  };

  function applyStartupState() {
    const s = settings.get();
    if (s.enabled && display && s.temperature && s.temperature !== 6500) {
      const r = setTemperature(s.temperature, { persist: false, preset: s.preset });
      console.log('[boot] 恢复上次色温:', JSON.stringify(r));
    }
    if (s.brightness < 100) {
      setBrightness(s.brightness, { persist: false });
      console.log('[boot] 恢复上次亮度: ' + s.brightness);
    }
  }

  // ==== IPC ====

  ipcMain.handle('app:get-version', () => app.getVersion());

  ipcMain.handle('display:set-temperature', (e, k) => setTemperature(k));

  ipcMain.handle('display:get-state', () => ({
    available: !!display,
    enabled: settings ? settings.get().enabled : true,
    temperature: display ? display.current.temperature : 6500,
    brightness: overlay ? overlay.brightness : 100
  }));

  ipcMain.handle('display:restore', () => restoreColor());

  ipcMain.handle('display:set-brightness', (e, b) => setBrightness(b));

  ipcMain.handle('settings:get', () => (settings ? settings.get() : null));

  ipcMain.handle('settings:set', (e, patch) => {
    if (!settings) return null;
    return settings.save(isPlainPatch(patch));
  });

  function isPlainPatch(patch) {
    return patch && typeof patch === 'object' && !Array.isArray(patch) ? patch : {};
  }

  // ==== 启动 ====

  app.whenReady().then(() => {
    console.log('[boot] app ready');
    initSettings();
    initDisplay();
    overlay = new BrightnessOverlay();
    createMainWindow();

    const { createTray } = require('./tray');
    tray = createTray({
      onToggleWindow: toggleWindow,
      onQuit: quitApp,
      actions: trayActions
    });
    console.log('[boot] tray created');

    applyStartupState();

    // 全局热键：Control+Alt+↑/↓ 色温 ±200K
    const hk = registerHotkeys(
      {
        onTempUp: () => nudgeTemperature(+200),
        onTempDown: () => nudgeTemperature(-200)
      },
      settings.get().hotkeys
    );
    if (hk.failed.length) console.warn('[boot] 热键注册失败:', hk.failed.join(','));

    // 屏幕参数变化 → 遮罩重定位
    screen.on('display-metrics-changed', () => {
      if (overlay) overlay.refit();
    });

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
    unregisterAll();
    if (overlay) overlay.dispose();
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
