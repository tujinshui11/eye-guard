'use strict';

const { Tray, Menu, nativeImage } = require('electron');
const path = require('path');
const { MODES } = require('./modes');

let tray = null;

/**
 * 创建托盘与菜单（v2：7 档场景模式子菜单 + 色温微调 + 恢复原色 + 设置 / 退出）
 * 模式表来自 modes.js（数值依据 docs/eye-parameters-research.md；冷白档为 v2 新增）
 * @param {{ onToggleWindow: Function, onQuit: Function, actions: object }} handlers
 */
function createTray({ onToggleWindow, onQuit, actions }) {
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
  tray.on('click', onToggleWindow);

  const rebuild = () => {
    if (!tray || tray.isDestroyed()) return;
    const state = actions.getState();

    const menu = Menu.buildFromTemplate([
      { label: '护眼助手', enabled: false },
      { type: 'separator' },
      ...MODES.map((m) => ({
        label: m.name + '（' + m.kelvin + 'K）',
        type: 'radio',
        checked: state.enabled && state.modeId === m.id,
        click: () => {
          actions.applyMode(m.id);
          rebuild();
        }
      })),
      { type: 'separator' },
      {
        label: '色温 +200K',
        click: () => {
          actions.tempDelta(+200);
          rebuild();
        }
      },
      {
        label: '色温 −200K',
        click: () => {
          actions.tempDelta(-200);
          rebuild();
        }
      },
      { type: 'separator' },
      {
        label: state.enabled ? '恢复原色（临时关闭）' : '重新启用护眼色彩',
        click: () => {
          if (state.enabled) actions.restoreColor();
          else actions.reenableColor();
          rebuild();
        }
      },
      {
        label: state.breakPaused ? '恢复提醒' : '暂停提醒 1 小时',
        click: () => {
          if (state.breakPaused) actions.resumeBreaks();
          else actions.pauseBreaks(1);
          rebuild();
        }
      },
      { type: 'separator' },
      { label: '设置…', click: onToggleWindow },
      { label: '退出', click: onQuit }
    ]);
    tray.setContextMenu(menu);
  };

  rebuild();
  console.log('[tray] registered');
  return tray;
}

module.exports = { createTray };
