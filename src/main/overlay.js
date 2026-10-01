'use strict';

/**
 * BrightnessOverlay：亮度调节（透明黑色遮罩层）
 *
 * 为什么不用 gamma：本机驱动「50% 规则」（W2 实测定档）下，
 * gamma 亮度会与色温争抢空间（亮度 50% 时色温完全失效）。
 * 遮罩方案将两个维度解耦：色温走 gamma（硬件级），亮度走遮罩（任意深度）。
 *
 * 特性：全屏透明窗口、点击穿透、不抢焦点、不进任务栏、置顶。
 * 已知限制：多显示器仅覆盖主屏（MVP）；部分独占全屏应用可能盖住遮罩。
 */

const { BrowserWindow, screen } = require('electron');
const path = require('path');

class BrightnessOverlay {
  constructor() {
    this.window = null;
    this.brightness = 100;
  }

  /** 主显示器逻辑边界（DIP） */
  _bounds() {
    const d = screen.getPrimaryDisplay();
    return d.bounds;
  }

  ensureWindow() {
    if (this.window && !this.window.isDestroyed()) return this.window;
    const { x, y, width, height } = this._bounds();

    this.window = new BrowserWindow({
      x,
      y,
      width,
      height,
      transparent: true,
      frame: false,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      focusable: false,
      hasShadow: false,
      show: false,
      enableLargerThanScreen: true,
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true
      }
    });

    this.window.setIgnoreMouseEvents(true, { forward: true });
    this.window.setAlwaysOnTop(true, 'screen-saver');
    this.window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
    this.window.loadFile(path.join(__dirname, '..', 'renderer', 'overlay.html'));

    this.window.on('closed', () => {
      this.window = null;
    });
    return this.window;
  }

  /**
   * 设置亮度（50-100）。100 = 隐藏遮罩；<100 = 显示 alpha=(100-b)/100 的黑色遮罩。
   * @returns {number} 实际生效亮度
   */
  setBrightness(brightness) {
    this.brightness = Math.min(100, Math.max(50, Number(brightness) || 100));
    const alpha = (100 - this.brightness) / 100;

    let win;
    try {
      win = this.ensureWindow();
    } catch (err) {
      console.warn('[overlay] 创建遮罩窗口失败:', err.message);
      return this.brightness;
    }

    if (alpha <= 0.001) {
      if (win.isVisible()) win.hide();
      return this.brightness;
    }

    const apply = () => {
      if (win.isDestroyed()) return;
      win.webContents
        .executeJavaScript(`window.setMaskAlpha && window.setMaskAlpha(${alpha})`)
        .catch(() => {});
    };

    if (win.webContents.isLoading()) {
      win.webContents.once('did-finish-load', apply);
    } else {
      apply();
    }
    if (!win.isVisible()) win.showInactive();
    return this.brightness;
  }

  /** 屏幕尺寸变化时重设边界 */
  refit() {
    if (!this.window || this.window.isDestroyed()) return;
    const { x, y, width, height } = this._bounds();
    this.window.setBounds({ x, y, width, height });
  }

  dispose() {
    if (this.window && !this.window.isDestroyed()) {
      this.window.destroy();
    }
    this.window = null;
  }
}

module.exports = { BrightnessOverlay };
