'use strict';

/**
 * ALS（环境光传感器）支持：Windows 上有 ALS 硬件的机器优先用真传感器测光。
 *
 * 实现方式：通过 PowerShell 桥接 WinRT `Windows.Devices.Sensors.LightSensor`：
 *   - 无原生依赖（Electron 44 无内建 ALS 接口，koffi 直调 WinRT 成本高）；
 *   - 单次采样 = 短命 PowerShell 进程（约 0.3–1s，windowsHide）；
 *   - 无硬件（GetDefault() = null）或读取失败时返回 available=false，
 *     由调用方（主进程感光监测）回退到摄像头测光。
 *
 * 输出协议（stdout 单行标记，便于跨 PowerShell 版本稳定解析）：
 *   ALS_NONE          —— 无 ALS 硬件
 *   ALS_NO_READING    —— 硬件存在但本次无读数
 *   ALS_LUX=<数值>    —— 有效读数（lux，可能为小数）
 */

const { execFile } = require('child_process');

const PS_TIMEOUT_MS = 8000;

const ALS_SCRIPT = [
  "$ErrorActionPreference='SilentlyContinue'",
  '$null = [Windows.Devices.Sensors.LightSensor, Windows.Devices.Sensors, ContentType=WindowsRuntime]',
  '$s = [Windows.Devices.Sensors.LightSensor]::GetDefault()',
  'if ($null -eq $s) {',
  "  Write-Output 'ALS_NONE'",
  '} else {',
  '  $r = $s.GetCurrentReading()',
  '  if ($null -eq $r) {',
  "    Write-Output 'ALS_NO_READING'",
  '  } else {',
  "    Write-Output ('ALS_LUX=' + [string][math]::Round([double]$r.IlluminanceInLux, 1))",
  '  }',
  '}'
].join('\n');

/**
 * 解析 PowerShell 输出 → { available, lux }。
 * @param {string|null} stdout
 * @returns {{ available: boolean, lux: number|null }}
 */
function parseAlsOutput(stdout) {
  if (typeof stdout !== 'string' || stdout.length === 0) {
    return { available: false, lux: null };
  }
  if (stdout.includes('ALS_NONE')) return { available: false, lux: null };
  const m = stdout.match(/ALS_LUX=(\d+(?:\.\d+)?)/);
  if (m) return { available: true, lux: Number(m[1]) };
  if (stdout.includes('ALS_NO_READING')) return { available: true, lux: null };
  return { available: false, lux: null };
}

/** 执行一次采样脚本；任何失败（超时/启动失败）都以空串返回，由解析层归类。 */
function runAlsScript() {
  return new Promise((resolve) => {
    execFile(
      'powershell.exe',
      ['-NoProfile', '-NonInteractive', '-Command', ALS_SCRIPT],
      { timeout: PS_TIMEOUT_MS, windowsHide: true },
      (err, stdout) => {
        resolve(typeof stdout === 'string' ? stdout : '');
      }
    );
  });
}

/** 读取一次 ALS：{ available, lux }。lux 为 null 表示本次无有效读数。 */
async function readAls() {
  const out = await runAlsScript();
  return parseAlsOutput(out);
}

/** 探测本机是否有可用的 ALS 硬件（读一次即知）。 */
async function detectAls() {
  const r = await readAls();
  return r.available;
}

module.exports = { ALS_SCRIPT, parseAlsOutput, readAls, detectAls };
