'use strict';

/**
 * 时间调度判定（纯函数模块）
 * 设计文档 §5.2：docs/superpowers/specs/2026-10-01--v2-design.md:137-148
 *
 * 无 I/O、无副作用、无定时器——仅做「此刻该触发哪条调度」的判定。
 * 生产侧的 setInterval(30s) tick 与 powerMonitor resume 钩子由调用方
 * （src/main/index.js）驱动，本模块可独立单测。
 *
 * 数据模型（settings.schedule）：
 *   { enabled:false, entries:[{id,time:"HH:MM",modeId,action:"auto"|"ask",enabled}], lastFired:{} }
 *   lastFired 以「时间戳」而非「日期」记录触发点，时钟回拨/睡眠唤醒下仍安全。
 */

/** "HH:MM"（24 小时制，严格两位）——秒/越界/带空白一律非法 */
const TIME_RE = /^([01]\d|2[0-3]):([0-5]\d)$/;

/**
 * "HH:MM" → 基于 now 日期的今日时间戳（本地时区，秒与毫秒归零）
 * @param {string} time 形如 "09:00"
 * @param {number|Date} now 基准时刻（取其所处「当日」）
 * @returns {number|null} 时间戳；格式非法返回 null
 */
function timeToTs(time, now) {
  if (typeof time !== 'string') return null;
  const m = TIME_RE.exec(time);
  if (!m) return null;
  const d = new Date(now);
  d.setHours(Number(m[1]), Number(m[2]), 0, 0);
  return d.getTime();
}

/**
 * 判定此刻是否有条目 due（到点触发）。
 *
 * 条目 due ⟺ 以下全部成立：
 *   - enabled !== false（默认视为启用）
 *   - time 合法，且 now ∈ [ts, ts + windowMs)（今日窗口，右开）
 *   - (lastFired[id] || 0) < ts（本窗口尚未触发）
 * 多条同时 due → 返回 ts 最新者；无 due 返回 null。
 *
 * @param {Array} entries 调度条目
 * @param {number} now 当前时间戳（毫秒）
 * @param {Object} lastFired id → 上次触发时间戳
 * @param {number} [windowMs=300000] 触发容忍窗口（默认 5 分钟）
 * @returns {Object|null} due 条目或 null
 */
function resolveDueEntry(entries, now, lastFired, windowMs = 300000) {
  const fired = lastFired || {};
  let best = null;
  let bestTs = -1;
  for (const e of entries || []) {
    if (!e || e.enabled === false) continue;
    const ts = timeToTs(e.time, now);
    if (ts === null) continue;
    if (now < ts || now >= ts + windowMs) continue;
    if ((fired[e.id] || 0) >= ts) continue;
    if (ts > bestTs) {
      bestTs = ts;
      best = e;
    }
  }
  return best;
}

/**
 * 启动/唤醒对齐：取今日已过（now >= ts）的最新条目并判定是否需静默补应用。
 *
 * 语义（设计文档 §5.2）：
 *   - 只看「今天已过的最新一条」（不限 action；启用中、time 合法），据此决策，不回退到更早条目；
 *   - 该条 action === 'auto' 且今日窗口未触发 → 返回该条目（调用方静默应用并 markFired）；
 *   - 该条为 ask、或已触发 → 返回 null（ask 错过不补弹，避免开机即打扰）；
 *   - 今日无已过条目 → null。
 *
 * @param {Array} entries 调度条目
 * @param {number} now 当前时间戳
 * @param {Object} lastFired id → 上次触发时间戳
 * @returns {Object|null} 需静默应用的 auto 条目或 null
 */
function alignStartup(entries, now, lastFired) {
  const fired = lastFired || {};
  let latest = null;
  let latestTs = -1;
  for (const e of entries || []) {
    if (!e || e.enabled === false) continue;
    const ts = timeToTs(e.time, now);
    if (ts === null) continue;
    if (now < ts) continue;
    if (ts > latestTs) {
      latestTs = ts;
      latest = e;
    }
  }
  if (!latest) return null;
  if (latest.action !== 'auto') return null;
  if ((fired[latest.id] || 0) >= latestTs) return null;
  return latest;
}

/**
 * 记录触发：返回含 lastFired[id]=now 的新对象（不修改入参，便于持久化与回滚）
 * @param {Object} lastFired 原映射（可为 null）
 * @param {string} entryId 条目 id
 * @param {number} now 触发时间戳
 * @returns {Object} 新映射
 */
function markFired(lastFired, entryId, now) {
  return { ...(lastFired || {}), [entryId]: now };
}

module.exports = {
  timeToTs,
  resolveDueEntry,
  alignStartup,
  markFired
};
