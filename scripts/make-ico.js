'use strict';

// 从 assets/icon-sizes/*.png 生成多尺寸 assets/icon.ico（PNG-in-ICO，Vista+）
// 并同步到 src-tauri/icons/icon.ico（应用打包用）
// 用法：node scripts/make-ico.js

const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const sizesDir = path.join(root, 'assets', 'icon-sizes');
const outPaths = [
  path.join(root, 'assets', 'icon.ico'),
  path.join(root, 'src-tauri', 'icons', 'icon.ico')
];
const SIZES = [16, 24, 32, 48, 64, 128, 256];

const pngs = SIZES.map((size) => {
  const p = path.join(sizesDir, `${size}.png`);
  if (!fs.existsSync(p)) {
    console.error(`缺少 ${p}（先运行 scripts/make-icon.ps1）`);
    process.exit(1);
  }
  return { size, data: fs.readFileSync(p) };
});

// ICONDIR (6 bytes): reserved=0, type=1(icon), count=N
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(pngs.length, 4);

// ICONDIRENTRY * N (16 bytes each)，256 的宽高字节写 0
let offset = 6 + 16 * pngs.length;
const entries = [];
const blobs = [];
for (const { size, data } of pngs) {
  const e = Buffer.alloc(16);
  e.writeUInt8(size >= 256 ? 0 : size, 0); // width
  e.writeUInt8(size >= 256 ? 0 : size, 1); // height
  e.writeUInt8(0, 2); // color count
  e.writeUInt8(0, 3); // reserved
  e.writeUInt16LE(1, 4); // planes
  e.writeUInt16LE(32, 6); // bpp
  e.writeUInt32LE(data.length, 8); // PNG 数据大小
  e.writeUInt32LE(offset, 12); // 数据偏移
  entries.push(e);
  blobs.push(data);
  offset += data.length;
}

const ico = Buffer.concat([header, ...entries, ...blobs]);
for (const out of outPaths) {
  fs.writeFileSync(out, ico);
  console.log(`icon.ico 已生成: ${out}（${pngs.length} 尺寸: ${SIZES.join('/')}，${ico.length} bytes）`);
}
