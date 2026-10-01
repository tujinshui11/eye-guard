'use strict';

// W1：验证 contextBridge 链路（显示版本号）；后续波次在此接线色温/亮度/提醒
window.addEventListener('DOMContentLoaded', async () => {
  try {
    const version = await window.eyeGuard.getVersion();
    document.getElementById('version').textContent = 'v' + version;
    console.log('[renderer] IPC bridge OK, version =', version);
  } catch (err) {
    console.warn('[renderer] IPC bridge not ready:', err && err.message);
  }
});
