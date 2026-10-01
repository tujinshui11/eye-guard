'use strict';

/**
 * 全局热键（design.md:70）：
 *  - Ctrl+Alt+↑ / Ctrl+Alt+↓ 调节色温 ±200K
 *  - 注册失败不阻塞启动（返回失败列表，UI 提示）
 */

const { globalShortcut } = require('electron');

/**
 * @param {{ onTempUp: Function, onTempDown: Function }} handlers
 * @param {{ tempUp?: string, tempDown?: string }} [keys]
 * @returns {{ registered: Record<string, boolean>, failed: string[] }}
 */
function registerHotkeys(handlers, keys = {}) {
  const accels = {
    tempUp: keys.tempUp || 'Control+Alt+Up',
    tempDown: keys.tempDown || 'Control+Alt+Down'
  };
  const fns = {
    tempUp: handlers.onTempUp,
    tempDown: handlers.onTempDown
  };

  const registered = {};
  const failed = [];

  for (const name of Object.keys(accels)) {
    if (typeof fns[name] !== 'function') continue;
    let ok = false;
    try {
      ok = globalShortcut.register(accels[name], fns[name]);
    } catch (err) {
      console.warn(`[hotkeys] 注册 ${name} (${accels[name]}) 异常:`, err.message);
    }
    registered[name] = ok;
    if (!ok) failed.push(name);
  }

  console.log(
    '[hotkeys] 注册结果:',
    JSON.stringify(registered),
    failed.length ? '失败: ' + failed.join(',') : ''
  );
  return { registered, failed };
}

function unregisterAll() {
  globalShortcut.unregisterAll();
}

module.exports = { registerHotkeys, unregisterAll };
