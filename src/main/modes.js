'use strict';

/**
 * 场景模式表（纯数据模块）
 * 设计文档 §4.2：docs/superpowers/specs/2026-10-01--v2-design.md
 *
 * 每档 = { 色温 kelvin(K), 亮度 brightness(%) } 的组合，应用管线
 * applyMode(modeId) 将其落到 setTemperature(k) + setBrightness(b)。
 *
 * 数值依据：docs/eye-parameters-research.md
 *   - natural  6500K/100：白天与色彩工作的原色基线（调研 §2.1，f.lux 白天不干预；
 *                        白天蓝光有益于警觉与节律同步，刻意不压）
 *   - office   5500K/100：办公照明 CCT 建议上限（调研 §2.6，一般办公 4000–5500K）
 *   - reading  5000K/ 90：纸感、低反差阅读（亮度略降以匹配环境光）
 *   - evening  4500K/ 80：f.lux 昼→夜过渡区间（调研 §2.1）
 *   - night    3400K/ 70：f.lux 默认日落值（调研 §2.1，与之精确一致）
 *   - deepnight 2700K/60：睡前档（调研 §2.3 melanopic 剂量控制方向；
 *                        本机受驱动 50% 规则钳制，UI 需如实回报实际生效色温）
 *   - focus    8000K/100：**冷区新增档（产品取舍）**。依据方向为「白天冷光提高警觉」
 *                        的照明常识，非临床结论；色温域已在 W6 扩展至 2000–10000K。
 *
 * 模块零依赖、无副作用，可独立单测。
 */

/** 7 档场景模式；数组顺序即 UI 网格呈现顺序（2 列 × 4 行，末格为自定义态） */
const MODES = [
  { id: 'natural', name: '原色', kelvin: 6500, brightness: 100 },
  { id: 'focus', name: '冷白提神', kelvin: 8000, brightness: 100 },
  { id: 'office', name: '办公', kelvin: 5500, brightness: 100 },
  { id: 'reading', name: '阅读暖白', kelvin: 5000, brightness: 90 },
  { id: 'evening', name: '傍晚', kelvin: 4500, brightness: 80 },
  { id: 'night', name: '夜晚', kelvin: 3400, brightness: 70 },
  { id: 'deepnight', name: '深夜', kelvin: 2700, brightness: 60 }
];

/** 默认模式：不干预的原色档（白天/色彩工作场景） */
const DEFAULT_MODE_ID = 'natural';

/**
 * 按 id 查模式
 * @param {string} id 模式 id
 * @returns {{id:string,name:string,kelvin:number,brightness:number}|null} 命中返回该档，未命中返回 null
 */
function getMode(id) {
  for (const mode of MODES) {
    if (mode.id === id) return mode;
  }
  return null;
}

module.exports = {
  MODES,
  getMode,
  DEFAULT_MODE_ID
};
