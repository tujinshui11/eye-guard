'use strict';

/**
 * koffi FFI 底层封装：gdi32 Get/SetDeviceGammaRamp
 *
 * 探测记录（2026-10-01，探针 .rivet/scratch/gamma-probe/probe.js）：
 *   koffi 3.3.2 → gdi32 读写链路 PASS（B[255] 65535→42598 精确匹配，读回一致）
 *   Electron 44.5.1 主进程内同样 PASS（N-API 10）
 *
 * 注意：屏幕 DC 在模块内缓存复用（避免高频 GetDC 泄漏 GDI 句柄）。
 */

let cachedHdc = null;
let loadError = null;
let api = null;

function ensureLoaded() {
  if (loadError) throw loadError;
  if (api) return api;
  try {
    const koffi = require('koffi');
    const gdi32 = koffi.load('gdi32.dll');
    const user32 = koffi.load('user32.dll');
    api = {
      GetDC: user32.func('void *GetDC(void *hWnd)'),
      GetDeviceGammaRamp: gdi32.func('int GetDeviceGammaRamp(void *hdc, uint16_t *ramp)'),
      SetDeviceGammaRamp: gdi32.func('int SetDeviceGammaRamp(void *hdc, uint16_t *ramp)')
    };
    return api;
  } catch (err) {
    loadError = new Error('加载 gdi32 (koffi FFI) 失败：' + err.message);
    throw loadError;
  }
}

function getScreenDC() {
  const a = ensureLoaded();
  if (cachedHdc) return cachedHdc;
  cachedHdc = a.GetDC(null);
  if (!cachedHdc) throw new Error('GetDC(null) 返回空句柄');
  return cachedHdc;
}

/**
 * 读取当前 gamma ramp
 * @returns {Uint16Array} 768 项（R0..R255, G0..G255, B0..B255，每项 0-65535）
 * @throws 驱动不支持 / FFI 失败时抛出
 */
function readRamp() {
  const a = ensureLoaded();
  const hdc = getScreenDC();
  const buf = new Uint16Array(768);
  const ok = a.GetDeviceGammaRamp(hdc, buf);
  if (!ok) throw new Error('GetDeviceGammaRamp 调用失败（显卡驱动不支持 gamma 读取）');
  return buf;
}

/**
 * 写入 gamma ramp
 * @param {Uint16Array} lut 768 项
 * @returns {boolean} true = 驱动接受；false = 驱动拒绝（如超出支持范围）
 */
function writeRamp(lut) {
  const a = ensureLoaded();
  const hdc = getScreenDC();
  return !!a.SetDeviceGammaRamp(hdc, lut);
}

module.exports = { readRamp, writeRamp };
