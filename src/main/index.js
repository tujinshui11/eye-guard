'use strict';

const { app, BrowserWindow, ipcMain, screen } = require('electron');
const path = require('path');
const { DisplayController } = require('./display');
const { SettingsStore } = require('./settings');
const { BrightnessOverlay } = require('./overlay');
const { registerHotkeys, unregisterAll } = require('./hotkeys');
const { BreakTimer } = require('./breakTimer');

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
  let breakTimer = null;
  let breakWindow = null;
  let breakTickTimer = null;
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

  // ==== 设置 / 显示 ====

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

  // ==== 休息提醒 ====

  function initBreakSystem() {
    const b = settings.get().breaks;
    breakTimer = new BreakTimer({
      onEvent: (e) => handleBreakEvent(e)
    });
    breakTimer.configure({ workSeconds: b.workSeconds, breakSeconds: b.breakSeconds });

    if (b.enabled) {
      breakTimer.start();
      const pausedUntil = settings.get().pausedUntil;
      if (pausedUntil && pausedUntil > Date.now()) {
        breakTimer.pause(pausedUntil);
        console.log('[break] 已按上次暂停状态恢复（至 ' + new Date(pausedUntil).toLocaleTimeString() + '）');
      }
    }

    breakTickTimer = setInterval(() => {
      if (!breakTimer) return;
      breakTimer.tick();
      const st = breakTimer.getState();
      if (
        breakWindow &&
        !breakWindow.isDestroyed() &&
        (st.state === 'alerting' || st.state === 'resting')
      ) {
        breakWindow.webContents.send('break:update', st);
      }
    }, 1000);

    console.log(
      '[break] system initialized, enabled=' + b.enabled +
      ', work=' + b.workSeconds + 's, break=' + b.breakSeconds + 's, style=' + b.style
    );
  }

  function handleBreakEvent(e) {
    console.log('[break] event: ' + e.type);
    switch (e.type) {
      case 'alerting':
        showBreakWindow();
        break;
      case 'resting':
        pushBreakUpdate();
        break;
      case 'paused':
      case 'working':
      case 'skipped':
        hideBreakWindow();
        break;
      default:
        break;
    }
  }

  function showBreakWindow() {
    if (breakWindow && !breakWindow.isDestroyed()) {
      pushBreakUpdate();
      return;
    }
    const style = (settings && settings.get().breaks.style) || 'gentle';

    const common = {
      frame: false,
      skipTaskbar: true,
      alwaysOnTop: true,
      resizable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      show: false,
      webPreferences: {
        preload: path.join(__dirname, '..', 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true
      }
    };

    if (style === 'fullscreen') {
      const d = screen.getPrimaryDisplay();
      breakWindow = new BrowserWindow({
        ...common,
        x: d.bounds.x,
        y: d.bounds.y,
        width: d.bounds.width,
        height: d.bounds.height
      });
      breakWindow.setAlwaysOnTop(true, 'screen-saver');
      breakWindow.loadFile(path.join(__dirname, '..', 'renderer', 'break.html'));
      breakWindow.once('ready-to-show', () => {
        breakWindow.show();
        console.log('[break] 全屏提醒窗口已显示');
      });
    } else {
      breakWindow = new BrowserWindow({
        ...common,
        width: 420,
        height: 320,
        center: true
      });
      breakWindow.setAlwaysOnTop(true, 'screen-saver');
      breakWindow.loadFile(path.join(__dirname, '..', 'renderer', 'break.html'));
      breakWindow.once('ready-to-show', () => {
        breakWindow.showInactive(); // H4：不抢键盘焦点
        console.log('[break] 温和提醒窗口已显示（showInactive）');
      });
    }

    breakWindow.on('closed', () => {
      breakWindow = null;
    });
    breakWindow.webContents.on('did-finish-load', () => pushBreakUpdate());
  }

  function pushBreakUpdate() {
    if (!breakWindow || breakWindow.isDestroyed() || !breakTimer) return;
    breakWindow.webContents.send('break:update', breakTimer.getState());
  }

  function hideBreakWindow() {
    if (breakWindow && !breakWindow.isDestroyed()) {
      breakWindow.close();
    }
    breakWindow = null;
  }

  function pauseBreaks(hours = 1) {
    if (!breakTimer) return false;
    const until = Date.now() + hours * 3600 * 1000;
    if (settings) settings.save({ pausedUntil: until });
    const ok = breakTimer.pause(until);
    hideBreakWindow();
    console.log('[break] 暂停提醒 ' + hours + ' 小时（至 ' + new Date(until).toLocaleTimeString() + '）');
    return ok;
  }

  function resumeBreaks() {
    if (!breakTimer) return false;
    if (settings) settings.save({ pausedUntil: null });
    const ok = breakTimer.resume();
    console.log('[break] 已恢复提醒（state=' + breakTimer.getState().state + '）');
    return ok;
  }

  // ==== 托盘聚合操作 ====

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
      enabled: settings ? settings.get().enabled : true,
      breakPaused: breakTimer ? breakTimer.getState().state === 'paused' : false
    }),
    pauseBreaks,
    resumeBreaks
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
    const clean = patch && typeof patch === 'object' && !Array.isArray(patch) ? patch : {};
    const next = settings.save(clean);

    // 联动：breaks 配置变更 → 重配置状态机
    if (clean.breaks && breakTimer) {
      breakTimer.configure({
        workSeconds: next.breaks.workSeconds,
        breakSeconds: next.breaks.breakSeconds
      });
      if (next.breaks.enabled) {
        if (breakTimer.getState().state === 'idle') breakTimer.start();
      } else {
        breakTimer.stop();
        hideBreakWindow();
      }
      console.log('[break] 配置已更新并联动');
    }
    return next;
  });

  ipcMain.handle('break:get-state', () => (breakTimer ? breakTimer.getState() : null));

  ipcMain.handle('break:action', (e, name) => {
    if (!breakTimer) return false;
    switch (name) {
      case 'beginRest':
        return breakTimer.beginRest();
      case 'postpone':
        return breakTimer.postpone();
      case 'skip':
        return breakTimer.skip();
      case 'pause1h':
        return pauseBreaks(1);
      case 'resume':
        return resumeBreaks();
      default:
        return false;
    }
  });

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
    initBreakSystem();

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
    if (breakTickTimer) {
      clearInterval(breakTickTimer);
      breakTickTimer = null;
    }
    hideBreakWindow();
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
