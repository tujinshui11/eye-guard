'use strict';

const { Tray, Menu, nativeImage } = require('electron');
const path = require('path');

let tray = null;

/**
 * 创建托盘图标与基础菜单（W1 版本：设置 / 退出；后续波次扩展）。
 * @param {{ onToggleWindow: Function, onQuit: Function }} handlers
 */
function createTray({ onToggleWindow, onQuit }) {
  const iconPath = path.join(__dirname, '..', '..', 'assets', 'icon.png');
  let image = nativeImage.createFromPath(iconPath);
  if (image.isEmpty()) {
    console.warn('[tray] icon missing at ' + iconPath + '（使用空图标兜底）');
    image = nativeImage.createEmpty();
  } else {
    image = image.resize({ width: 16, height: 16 });
  }

  tray = new Tray(image);
  tray.setToolTip('护眼助手');
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: '打开设置面板', click: onToggleWindow },
    { type: 'separator' },
    { label: '退出', click: onQuit }
  ]));
  // 左键单击：切换设置面板显隐
  tray.on('click', onToggleWindow);

  console.log('[tray] registered');
  return tray;
}

module.exports = { createTray };
