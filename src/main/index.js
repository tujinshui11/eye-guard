'use strict';

const { app, BrowserWindow, ipcMain, screen, session, powerMonitor } = require('electron');
const path = require('path');
const { DisplayController } = require('./display');
const { SettingsStore } = require('./settings');
const { BrightnessOverlay } = require('./overlay');
const { registerHotkeys, unregisterAll } = require('./hotkeys');
const { BreakTimer } = require('./breakTimer');
const { MODES, getMode } = require('./modes');
const { resolveDueEntry, alignStartup, markFired } = require('./scheduler');
const { LumaAnalyzer } = require('./ambient');
const { detectAls, readAls } = require('./als');

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
  let scheduleTickTimer = null;
  let proposeWindow = null;
  let proposeTimer = null;
  let proposePayload = null;
  let cameraWindow = null;
  let ambientTimer = null;
  let ambientAnalyzer = null;
  let ambientFailures = 0;
  let ambientStoppedReason = null;
  let ambientSource = null; // 'als'（硬件光感）| 'camera'（摄像头）——探测完成后确定
  let ambientSession = 0; // 会话令牌：stop/重启后使在途的 ALS 探测失效
  let quitting = false;

  // ==== 窗口 ====

  function createMainWindow() {
    mainWindow = new BrowserWindow({
      width: 440,
      height: 680,
      show: false,
      backgroundColor: '#0B0F1A',
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

  /** 应用色温（可选持久化；modeId：手动调节置 custom，模式应用由 applyMode 统一持盘） */
  function setTemperature(k, { persist = true, modeId = 'custom', silent = false } = {}) {
    if (!display) return { ok: false, error: '当前环境不支持屏幕色温调节' };
    const t = Math.min(10000, Math.max(2000, Number(k) || 6500));
    let res;
    try {
      res = display.apply(t);
    } catch (err) {
      return { ok: false, error: err.message };
    }
    if (res.ok && persist && settings) {
      settings.save({ temperature: res.effectiveTemperature, modeId });
    }
    if (res.ok && !silent) broadcastDisplayChanged();
    return res;
  }

  function nudgeTemperature(delta) {
    const cur = display ? display.current.temperature : 6500;
    const next = Math.min(10000, Math.max(2000, cur + delta));
    console.log('[temp] nudge ' + delta + ' -> ' + next);
    return setTemperature(next);
  }

  function setBrightness(b, { persist = true, modeId = 'custom', silent = false } = {}) {
    if (!overlay) return { ok: false, brightness: 100 };
    const v = overlay.setBrightness(b);
    if (persist && settings) settings.save({ brightness: v, modeId });
    if (!silent) broadcastDisplayChanged();
    return { ok: true, brightness: v };
  }

  function restoreColor() {
    if (settings) settings.save({ enabled: false });
    if (!display) return false;
    try {
      const ok = display.restore();
      broadcastDisplayChanged();
      return ok;
    } catch (err) {
      console.error('[display] restore 失败:', err.message);
      return false;
    }
  }

  function reenableColor() {
    if (settings) settings.save({ enabled: true });
    const s = settings ? settings.get() : null;
    const mode = s && getMode(s.modeId);
    if (mode) return applyMode(s.modeId);
    return setTemperature(s ? s.temperature : 4500, { persist: false });
  }

  /**
   * 应用场景模式（v2 单一入口：UI 网格 / 托盘 / 调度 / 感光共用）
   * 模式 = 色温 + 亮度组合；应用即隐式启用护眼色彩
   */
  function applyMode(modeId, { persist = true, silent = false } = {}) {
    const mode = getMode(modeId);
    if (!mode) return { ok: false, error: '未知模式：' + modeId };
    const tRes = setTemperature(mode.kelvin, { persist: false, silent: true });
    setBrightness(mode.brightness, { persist: false, silent: true });
    if (persist && settings) {
      settings.save({
        enabled: true,
        modeId,
        temperature: display ? display.current.temperature : mode.kelvin,
        brightness: overlay ? overlay.brightness : mode.brightness
      });
    }
    if (!silent) broadcastDisplayChanged();
    return {
      ok: !!tRes.ok,
      mode: Object.assign({}, mode),
      temperature: display ? display.current.temperature : mode.kelvin,
      brightness: overlay ? overlay.brightness : mode.brightness
    };
  }

  /** 广播显示状态（模式 / 色温 / 亮度 / enabled）→ 各窗口 UI 回填 */
  function broadcastDisplayChanged() {
    const s = settings ? settings.get() : null;
    const payload = {
      temperature: display ? display.current.temperature : 6500,
      brightness: overlay ? overlay.brightness : 100,
      modeId: s ? s.modeId : 'natural',
      enabled: s ? s.enabled : true
    };
    for (const win of [mainWindow, breakWindow]) {
      if (win && !win.isDestroyed()) win.webContents.send('display:changed', payload);
    }
  }

  /** 广播主题变化 → 各窗口切换 data-theme */
  function broadcastThemeChanged(theme) {
    for (const win of [mainWindow, breakWindow]) {
      if (win && !win.isDestroyed()) win.webContents.send('theme:changed', theme);
    }
  }

  /** 开机自启（H3：写入后回读校验） */
  function setAutoLaunch(enabled) {
    try {
      app.setLoginItemSettings({
        openAtLogin: !!enabled,
        args: ['--hidden']
      });
      // 回读必须与写入使用【相同的 args】：Windows 端按「exe 路径 + 参数」完整字符串精确比对，
      // 缺 --hidden 会永远读回 false（表现为：点勾选框后立即弹回）。真机实测见 .rivet/scratch/autolaunch-probe。
      const check = app.getLoginItemSettings({ args: ['--hidden'] });
      console.log(
        '[autoLaunch] 设置 openAtLogin=' + !!enabled +
        ' | 回读=' + check.openAtLogin +
        ' | args=' + JSON.stringify(check.args || [])
      );
      if (settings) settings.save({ autoLaunch: !!enabled });
      return { ok: true, openAtLogin: check.openAtLogin, args: check.args || [] };
    } catch (err) {
      console.error('[autoLaunch] 设置失败:', err.message);
      return { ok: false, error: err.message };
    }
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
      transparent: true,
      backgroundColor: '#00000000',
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

  // ==== 建议卡片（调度询问 / 感光提醒共用窗口） ====

  function showPropose(payload) {
    proposePayload = payload;
    if (!proposeWindow || proposeWindow.isDestroyed()) {
      proposeWindow = new BrowserWindow({
        width: 400,
        height: 190,
        frame: false,
        transparent: true,
        backgroundColor: '#00000000',
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
      });
      proposeWindow.setAlwaysOnTop(true, 'screen-saver');
      proposeWindow.loadFile(path.join(__dirname, '..', 'renderer', 'propose.html'));
      proposeWindow.once('ready-to-show', () => {
        if (proposeWindow && !proposeWindow.isDestroyed()) proposeWindow.showInactive();
      });
      proposeWindow.webContents.on('did-finish-load', () => {
        if (proposeWindow && !proposeWindow.isDestroyed() && proposePayload) {
          proposeWindow.webContents.send('propose:update', proposePayload);
        }
      });
      proposeWindow.on('closed', () => {
        proposeWindow = null;
      });
    } else {
      proposeWindow.webContents.send('propose:update', payload);
    }
    // 60 秒无操作自动关闭（视为忽略）
    if (proposeTimer) clearTimeout(proposeTimer);
    proposeTimer = setTimeout(() => resolvePropose('timeout'), 60 * 1000);
    console.log('[propose] shown: ' + payload.title);
  }

  /** 卡片交互结果：apply=切换；其余（dismiss/mute/timeout）不应用。窗口已消费即防重 */
  function resolvePropose(action) {
    if (proposeTimer) {
      clearTimeout(proposeTimer);
      proposeTimer = null;
    }
    const payload = proposePayload;
    proposePayload = null;
    if (proposeWindow && !proposeWindow.isDestroyed()) proposeWindow.close();
    if (action === 'apply' && payload && payload.modeId) {
      const r = applyMode(payload.modeId);
      console.log('[propose] apply ' + payload.modeId + ' → ' + JSON.stringify(r && r.ok));
    } else {
      console.log('[propose] resolved: ' + action);
    }
    return true;
  }

  // ==== 时间调度 ====

  function handleScheduleEntry(entry, now) {
    const mode = getMode(entry.modeId);
    if (!mode) return;
    // 触发即持久化 lastFired（本窗口消费，防重跨重启）
    const cur = settings.get().schedule;
    const nextFired = markFired(cur.lastFired, entry.id, now);
    settings.save({ schedule: { lastFired: nextFired } });

    if (entry.action === 'ask') {
      showPropose({
        kind: 'schedule',
        modeId: entry.modeId,
        title: '现在是 ' + entry.time,
        body: '切换到「' + mode.name + ' ' + mode.kelvin + 'K」吗？'
      });
    } else {
      const r = applyMode(entry.modeId);
      console.log('[schedule] auto → ' + entry.modeId + ' ok=' + (r && r.ok));
      notifyBalloon('已切换到「' + mode.name + '」');
    }
  }

  function checkSchedule(now) {
    if (!settings) return;
    const sch = settings.get().schedule;
    if (!sch || !sch.enabled || !Array.isArray(sch.entries)) return;
    const entry = resolveDueEntry(sch.entries, now, sch.lastFired || {});
    if (entry) handleScheduleEntry(entry, now);
  }

  /** 启动 / 唤醒对齐：今日已过的最新 auto 条目未触发 → 静默应用（ask 错过不补） */
  function alignScheduleOnBoot(reason) {
    if (!settings) return;
    const sch = settings.get().schedule;
    if (!sch || !sch.enabled || !Array.isArray(sch.entries)) return;
    const now = Date.now();
    const entry = alignStartup(sch.entries, now, sch.lastFired || {});
    if (!entry) return;
    const mode = getMode(entry.modeId);
    if (!mode) return;
    const nextFired = markFired(sch.lastFired || {}, entry.id, now);
    settings.save({ schedule: { lastFired: nextFired } });
    applyMode(entry.modeId, { silent: true });
    console.log('[schedule] 对齐（' + reason + '）：' + entry.time + ' → ' + entry.modeId);
  }

  function initScheduler() {
    scheduleTickTimer = setInterval(() => checkSchedule(Date.now()), 30 * 1000);
    console.log('[schedule] tick 已启动（30s）');
  }

  // ==== 感光监测（摄像头短开测光，默认关闭） ====

  function ensureCameraWindow() {
    if (cameraWindow && !cameraWindow.isDestroyed()) return cameraWindow;
    cameraWindow = new BrowserWindow({
      show: false,
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        backgroundThrottling: false
      }
    });
    cameraWindow.loadFile(path.join(__dirname, '..', 'renderer', 'camera.html'));
    cameraWindow.on('closed', () => {
      cameraWindow = null;
    });
    console.log('[ambient] 测光窗口已创建（隐藏）');
    return cameraWindow;
  }

  async function sampleAmbient() {
    const win = ensureCameraWindow();
    try {
      const res = await win.webContents.executeJavaScript('window.__sample()');
      if (res && typeof res === 'object' && typeof res.ok === 'boolean') return res;
      return { ok: false, error: '无效采样返回' };
    } catch (err) {
      return { ok: false, error: err.message };
    }
  }

  function getAmbientState() {
    const cfg = settings ? settings.get().ambient : null;
    return {
      enabled: cfg ? !!cfg.enabled : false,
      running: !!ambientTimer,
      source: ambientSource,
      failures: ambientFailures,
      stoppedReason: ambientStoppedReason,
      analyzer: ambientAnalyzer ? ambientAnalyzer.getState() : null
    };
  }

  function broadcastAmbientState() {
    const st = getAmbientState();
    if (mainWindow && !mainWindow.isDestroyed()) {
      mainWindow.webContents.send('ambient:state', st);
    }
  }

  function stopAmbientMonitor(reason) {
    ambientSession++; // 使在途的 ALS 探测失效
    if (ambientTimer) {
      clearInterval(ambientTimer);
      ambientTimer = null;
    }
    ambientSource = null;
    if (reason) ambientStoppedReason = reason;
    if (cameraWindow && !cameraWindow.isDestroyed()) {
      cameraWindow.close();
      cameraWindow = null;
    }
    broadcastAmbientState();
    if (reason) console.warn('[ambient] monitor stopped: ' + reason);
  }

  async function sampleAmbientOnce() {
    if (!ambientTimer) return;
    let res;
    if (ambientSource === 'als') {
      const r = await readAls();
      if (r.lux === null) {
        ambientFailures++;
        console.warn('[ambient] ALS 采样失败 ' + ambientFailures + '：' + (r.available ? '无读数' : '不可用'));
        if (ambientFailures >= 2) {
          // ALS 连续失败：降级摄像头（而非直接停止监测）
          ambientSource = 'camera';
          ambientFailures = 0;
          ensureCameraWindow();
          console.warn('[ambient] ALS 连续失败，已回退摄像头测光');
          broadcastAmbientState();
        }
        return;
      }
      res = { ok: true, luma: r.lux };
    } else {
      res = await sampleAmbient();
    }
    if (!res.ok) {
      ambientFailures++;
      console.warn('[ambient] 采样失败 ' + ambientFailures + '：' + res.error);
      if (ambientFailures >= 2) {
        stopAmbientMonitor('连续采样失败：' + res.error);
      }
      return;
    }
    ambientFailures = 0;
    if (!ambientAnalyzer) return;
    const evt = ambientAnalyzer.feed(res.luma, Date.now());
    if (evt === 'darker') handleAmbientDarker();
    broadcastAmbientState();
  }

  function handleAmbientDarker() {
    const cfg = settings ? settings.get().ambient : null;
    if (!cfg) return;
    const modeId = cfg.autoModeId || 'night';
    const mode = getMode(modeId);
    if (!mode) return;
    console.log('[ambient] 环境变暗（smooth=' + (ambientAnalyzer ? ambientAnalyzer.getState().smooth.toFixed(1) : '?') +
      ' baseline=' + (ambientAnalyzer ? ambientAnalyzer.getState().baseline.toFixed(1) : '?') + '）');
    if (cfg.action === 'auto') {
      applyMode(modeId);
      notifyBalloon('光线变暗，已切换到「' + mode.name + '」');
    } else {
      showPropose({
        kind: 'ambient',
        modeId,
        title: '环境光线变暗了',
        body: '切换到「' + mode.name + ' ' + mode.kelvin + 'K」吗？'
      });
    }
  }

  function startAmbientMonitor() {
    const cfg = settings.get().ambient || {};
    stopAmbientMonitor(null);
    ambientStoppedReason = null;
    ambientFailures = 0;
    if (!cfg.enabled) {
      console.log('[ambient] 未启用（默认关闭）');
      return;
    }
    ambientAnalyzer = new LumaAnalyzer({
      dropThresholdPercent: cfg.dropThresholdPercent || 35,
      cooldownMs: (cfg.cooldownMinutes || 15) * 60 * 1000
    });
    const session = ambientSession;
    const intervalMs = Math.max(30, Math.min(300, cfg.intervalSeconds || 60)) * 1000;
    // 先探测 ALS：有硬件用传感器（免摄像头、更准更快），无则回退摄像头
    detectAls().then((hasAls) => {
      if (session !== ambientSession) return; // 已被 stop/重启取代
      ambientSource = hasAls ? 'als' : 'camera';
      if (!hasAls) ensureCameraWindow();
      ambientTimer = setInterval(() => sampleAmbientOnce(), intervalMs);
      // ALS 可立即采样；摄像头路径首采延迟等测光窗口就绪
      setTimeout(() => sampleAmbientOnce(), hasAls ? 200 : 3000);
      console.log('[ambient] monitor started, source=' + ambientSource + ', interval=' + intervalMs + 'ms');
      broadcastAmbientState();
    });
  }

  /** 托盘气泡（尽力而为；不支持则静默） */
  function notifyBalloon(content) {
    if (tray && !tray.isDestroyed()) {
      try {
        tray.displayBalloon({ title: '护眼助手', content });
      } catch {
        /* 平台不支持，忽略 */
      }
    }
  }

  // ==== 托盘聚合操作 ====

  const trayActions = {
    applyMode: (modeId) => applyMode(modeId),
    tempDelta: nudgeTemperature,
    restoreColor,
    reenableColor,
    getState: () => ({
      temperature: display ? display.current.temperature : 6500,
      modeId: settings ? settings.get().modeId : 'natural',
      enabled: settings ? settings.get().enabled : true,
      breakPaused: breakTimer ? breakTimer.getState().state === 'paused' : false
    }),
    pauseBreaks,
    resumeBreaks
  };

  function applyStartupState() {
    const s = settings.get();
    if (!s.enabled) return;
    const mode = getMode(s.modeId);
    if (mode) {
      const r = applyMode(s.modeId, { persist: false, silent: true });
      console.log('[boot] 恢复模式 ' + s.modeId + '（' + mode.name + '）→ 生效 ' + r.temperature + 'K / ' + r.brightness + '%');
      return;
    }
    if (display && s.temperature && s.temperature !== 6500) {
      const r = setTemperature(s.temperature, { persist: false, silent: true });
      console.log('[boot] 恢复上次色温:', JSON.stringify(r));
    }
    if (s.brightness < 100) {
      setBrightness(s.brightness, { persist: false, silent: true });
      console.log('[boot] 恢复上次亮度: ' + s.brightness);
    }
  }

  // ==== IPC ====

  ipcMain.handle('app:get-version', () => app.getVersion());

  ipcMain.handle('display:set-temperature', (e, k) => setTemperature(k));

  ipcMain.handle('modes:list', () => MODES.map((m) => Object.assign({}, m)));

  ipcMain.handle('modes:apply', (e, modeId) => applyMode(modeId));

  ipcMain.handle('display:get-state', () => ({
    available: !!display,
    enabled: settings ? settings.get().enabled : true,
    temperature: display ? display.current.temperature : 6500,
    brightness: overlay ? overlay.brightness : 100,
    modeId: settings ? settings.get().modeId : 'natural'
  }));

  ipcMain.handle('display:restore', () => restoreColor());

  ipcMain.handle('display:set-brightness', (e, b) => setBrightness(b));

  ipcMain.handle('settings:get', () => (settings ? settings.get() : null));

  ipcMain.handle('settings:set', (e, patch) => {
    if (!settings) return null;
    const clean = patch && typeof patch === 'object' && !Array.isArray(patch) ? patch : {};
    const next = settings.save(clean);

    // 联动：主题变更 → 广播所有窗口（含发起窗口自身，幂等）
    if (clean.theme) broadcastThemeChanged(next.theme);

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

    // 联动：ambient 配置变更 → 重启感光监测（内部幂等：先停再按需启）
    if (clean.ambient) {
      startAmbientMonitor('settings');
    }
    return next;
  });

  ipcMain.handle('app:set-auto-launch', (e, enabled) => setAutoLaunch(enabled));

  ipcMain.handle('app:get-auto-launch', () => {
    try {
      // 与 setAutoLaunch 写入端一致的 args；否则 Windows 下永远读回 false
      const check = app.getLoginItemSettings({ args: ['--hidden'] });
      return { openAtLogin: check.openAtLogin, args: check.args || [] };
    } catch (err) {
      return { openAtLogin: false, args: [], error: err.message };
    }
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

  ipcMain.handle('propose:action', (e, name) => {
    resolvePropose(String(name || 'dismiss'));
    return true;
  });

  ipcMain.handle('ambient:get-state', () => getAmbientState());

  // ==== 启动 ====

  app.whenReady().then(() => {
    console.log('[boot] app ready');
    // 权限闸门（fail-closed）：仅放行 media（测光窗口 getUserMedia），其余一律拒绝
    session.defaultSession.setPermissionRequestHandler((wc, permission, callback) => {
      callback(permission === 'media');
    });
    session.defaultSession.setPermissionCheckHandler((wc, permission) => permission === 'media');

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
    initScheduler();
    alignScheduleOnBoot('boot');
    startAmbientMonitor('boot');

    // 睡眠唤醒 → 调度对齐（错过窗口的 auto 补应用；ask 不补弹）
    powerMonitor.on('resume', () => {
      console.log('[boot] system resumed');
      alignScheduleOnBoot('resume');
      // 唤醒后重启感光监测（环境可能已变，基线重新积累）
      if (ambientTimer) startAmbientMonitor('resume');
    });

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
    if (scheduleTickTimer) {
      clearInterval(scheduleTickTimer);
      scheduleTickTimer = null;
    }
    stopAmbientMonitor('quit');
    if (proposeTimer) {
      clearTimeout(proposeTimer);
      proposeTimer = null;
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
