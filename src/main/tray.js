'use strict';

const { Tray, Menu, nativeImage } = require('electron');
const path = require('path');

let tray = null;

// 预设档位（依据：docs/eye-parameters-research.md）
// - 办公 5500K：办公照明 CCT 建议（4000-5500K）上限，白天保留节律信号
// - 傍晚 4500K：f.lux 昼→夜过渡区间
// - 夜晚 3400K：f.lux 默认日落值（"卤素灯"色）
// - 深夜 2700K：睡前档（本机受驱动 50% 规则钳制约 3300K，UI 如实提示）
const PRESETS = [
  { name: 'office', label: '办公 5500K', k: 5500 },
  { name: 'evening', label: '傍晚 4500K', k: 4500 },
  { name: 'night', label: '夜晚 3400K', k: 3400 },
  { name: 'deepnight', label: '深夜 2700K', k: 2700 },
  { name: 'off', label: '关闭（原色）', k: 6500 }
];

/**
 * 创建托盘与菜单（W3：预设勾选 / 色温微调 / 恢复原色 / 设置 / 退出）
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
      ...PRESETS.map((p) => ({
        label: p.label,
        type: 'radio',
        checked: state.enabled && state.temperature === p.k,
        click: () => {
          actions.applyPreset(p.k, p.name);
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
