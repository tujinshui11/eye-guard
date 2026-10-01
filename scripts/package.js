'use strict';

// 打包脚本（便携版）：npm run package
//
// 使用 JS API + RegExp 字面量——避免经 CLI/Git-Bash 传正则的转义不确定性。
// 教训（2026-10-01）：MSYS2 会把参数里的 `\.` 转换成 `/`，
// 曾导致 `--ignore` 静默失效、产物从 ~250MB 膨胀到 907MB。
//
// 输出：dist/护眼助手-win32-x64/护眼助手.exe
// koffi 原生模块由 packager 默认 asar.unpack（**/*.node）自动解包。

const path = require('path');
const fs = require('fs');

// @electron/packager 为 ESM 包，函数是命名导出 packager
const packager = require('@electron/packager').packager;

const SRC = path.join(__dirname, '..');

const IGNORE = [
  /^[/\\]\.rivet([/\\]|$)/,
  /^[/\\]docs([/\\]|$)/,
  /^[/\\]tests([/\\]|$)/,
  /^[/\\]scripts([/\\]|$)/,
  /^[/\\]dist([/\\]|$)/,
  /^[/\\]\.git([/\\]|$)/
];

// 离线优先：vendor/ 内若备有对应版本的 electron zip，则完全跳过网络
// （本环境对 GitHub 连接时通时断——@electron/get 的校验文件下载也会超时）
const vendorZipDir = path.join(SRC, 'vendor');

packager({
  dir: SRC,
  name: '护眼助手',
  platform: 'win32',
  arch: 'x64',
  out: path.join(SRC, 'dist'),
  icon: path.join(SRC, 'assets', 'icon.ico'),
  overwrite: true,
  prune: true,
  electronZipDir: fs.existsSync(vendorZipDir) ? vendorZipDir : undefined,
  ignore: IGNORE
})
  .then((appPaths) => {
    console.log('[package] 完成:', appPaths.join(', '));
  })
  .catch((err) => {
    console.error('[package] 失败:', err);
    process.exit(1);
  });
