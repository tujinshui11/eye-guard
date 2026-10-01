'use strict';

// 从 assets/icon.png 生成 assets/icon.ico（PNG-in-ICO 格式，Vista+ 支持）
// 用法：node scripts/make-ico.js

const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const pngPath = path.join(root, 'assets', 'icon.png');
const icoPath = path.join(root, 'assets', 'icon.ico');

if (!fs.existsSync(pngPath)) {
  console.error('缺少 assets/icon.png（先运行 scripts/make-icon.ps1）');
  process.exit(1);
}

const png = fs.readFileSync(pngPath);

// ICONDIR (6 bytes)：reserved=0, type=1(icon), count=1
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(1, 4);

// ICONDIRENTRY (16 bytes)
const entry = Buffer.alloc(16);
entry.writeUInt8(0, 0); // width 256（0 表示 256）
entry.writeUInt8(0, 1); // height 256
entry.writeUInt8(0, 2); // color count
entry.writeUInt8(0, 3); // reserved
entry.writeUInt16LE(1, 4); // planes
entry.writeUInt16LE(32, 6); // bpp
entry.writeUInt32LE(png.length, 8); // PNG 数据大小
entry.writeUInt32LE(22, 12); // 数据偏移（6 + 16）

fs.writeFileSync(icoPath, Buffer.concat([header, entry, png]));
console.log('icon.ico 已生成:', icoPath, '（' + (22 + png.length) + ' bytes）');
