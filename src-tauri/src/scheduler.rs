//! 时间调度判定（纯函数模块）——对照移植 `src/main/scheduler.js`
//!
//! 无 I/O、无副作用、无定时器——仅做「此刻该触发哪条调度」的判定。
//! 生产侧 tick 与唤醒钩子由调用方（调度壳）驱动，本模块可独立单测。

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 触发容忍窗口默认值（5 分钟）
pub const DEFAULT_WINDOW_MS: i64 = 300_000;

/// 调度条目（settings.schedule.entries 的元素）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub id: String,
    pub time: String,
    #[serde(rename = "modeId", default)]
    pub mode_id: String,
    #[serde(default)]
    pub action: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// "HH:MM"（24 小时制，严格两位）——秒/越界/带空白一律非法
fn parse_hhmm(time: &str) -> Option<(u32, u32)> {
    let b = time.as_bytes();
    if b.len() != 5 || b[2] != b':' {
        return None;
    }
    let d = |x: u8| (x as char).to_digit(10);
    let h = d(b[0])? * 10 + d(b[1])?;
    let m = d(b[3])? * 10 + d(b[4])?;
    if h <= 23 && m <= 59 {
        Some((h, m))
    } else {
        None
    }
}

/// "HH:MM" → 基于 now 日期的今日时间戳（本地时区，秒与毫秒归零）
/// 格式非法返回 None
pub fn time_to_ts(time: &str, now_ms: i64) -> Option<i64> {
    let (h, m) = parse_hhmm(time)?;
    let now_local = Local.timestamp_millis_opt(now_ms).single()?;
    let date = now_local.date_naive();
    let naive = date.and_hms_opt(h, m, 0)?;
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp_millis())
}

/// 判定此刻是否有条目 due（到点触发）。
///
/// 条目 due ⟺ 以下全部成立：
///   - enabled（对照 JS 的 enabled !== false——缺省视为启用）
///   - time 合法，且 now ∈ [ts, ts + window_ms)（今日窗口，右开）
///   - (last_fired[id] || 0) < ts（本窗口尚未触发）
/// 多条同时 due → 返回 ts 最新者；无 due 返回 None。
pub fn resolve_due_entry<'a>(
    entries: &'a [ScheduleEntry],
    now: i64,
    last_fired: &HashMap<String, i64>,
    window_ms: i64,
) -> Option<&'a ScheduleEntry> {
    let mut best: Option<&ScheduleEntry> = None;
    let mut best_ts = -1i64;
    for e in entries {
        if !e.enabled {
            continue;
        }
        let Some(ts) = time_to_ts(&e.time, now) else {
            continue;
        };
        if now < ts || now >= ts + window_ms {
            continue;
        }
        if *last_fired.get(&e.id).unwrap_or(&0) >= ts {
            continue;
        }
        if ts > best_ts {
            best_ts = ts;
            best = Some(e);
        }
    }
    best
}

/// 启动/唤醒对齐：取今日已过（now >= ts）的最新条目并判定是否需静默补应用。
///
///   - 该条 action == "auto" 且今日窗口未触发 → 返回该条目（调用方静默应用并 mark_fired）；
///   - 该条为 ask、或已触发 → None（ask 错过不补弹）；
///   - 今日无已过条目 → None。
pub fn align_startup<'a>(
    entries: &'a [ScheduleEntry],
    now: i64,
    last_fired: &HashMap<String, i64>,
) -> Option<&'a ScheduleEntry> {
    let mut latest: Option<&ScheduleEntry> = None;
    let mut latest_ts = -1i64;
    for e in entries {
        if !e.enabled {
            continue;
        }
        let Some(ts) = time_to_ts(&e.time, now) else {
            continue;
        };
        if now < ts {
            continue;
        }
        if ts > latest_ts {
            latest_ts = ts;
            latest = Some(e);
        }
    }
    let latest = latest?;
    if latest.action != "auto" {
        return None;
    }
    if *last_fired.get(&latest.id).unwrap_or(&0) >= latest_ts {
        return None;
    }
    Some(latest)
}

/// 记录触发：返回含 last_fired[id]=now 的新映射（不修改入参）
pub fn mark_fired(
    last_fired: &HashMap<String, i64>,
    entry_id: &str,
    now: i64,
) -> HashMap<String, i64> {
    let mut out = last_fired.clone();
    out.insert(entry_id.to_string(), now);
    out
}

// ============================================================
// 测试——对照移植 tests/scheduler.test.js（假时钟注入）
// ============================================================
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate};

    /// 假时钟：基准日 2026-10-01（本地时区）。day_offset 便于构造昨日/明日。
    fn at(day_offset: i64, h: u32, mi: u32, s: u32) -> i64 {
        let base = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap() + Duration::days(day_offset);
        let naive = base.and_hms_opt(h, mi, s).unwrap();
        Local
            .from_local_datetime(&naive)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    fn now_10am() -> i64 {
        at(0, 10, 0, 0)
    }

    fn e1() -> ScheduleEntry {
        ScheduleEntry {
            id: "s1".into(),
            time: "09:00".into(),
            mode_id: "office".into(),
            action: "auto".into(),
            enabled: true,
        }
    }

    fn no_fired() -> HashMap<String, i64> {
        HashMap::new()
    }

    // ---------------------------------------------------------- time_to_ts

    #[test]
    fn time_to_ts_today() {
        let now = now_10am();
        assert_eq!(time_to_ts("09:00", now), Some(at(0, 9, 0, 0)));
        assert_eq!(time_to_ts("10:00", now), Some(now));
        assert_eq!(time_to_ts("00:00", now), Some(at(0, 0, 0, 0)));
        assert_eq!(time_to_ts("23:59", now), Some(at(0, 23, 59, 0)));
    }

    #[test]
    fn time_to_ts_same_day_regardless_of_now_time() {
        assert_eq!(time_to_ts("09:00", at(0, 0, 1, 0)), Some(at(0, 9, 0, 0)));
        assert_eq!(time_to_ts("09:00", at(0, 23, 59, 0)), Some(at(0, 9, 0, 0)));
    }

    #[test]
    fn time_to_ts_invalid_returns_none() {
        let now = now_10am();
        for bad in [
            "9:00", "09:0", "24:00", "09:60", "09:00:00", "0900", "", "abc", "0:0", " 09:00",
            "09:00 ",
        ] {
            assert_eq!(time_to_ts(bad, now), None, "非法时间 {bad:?} 应返回 None");
        }
    }

    // ------------------------------------------------------ resolve_due_entry

    #[test]
    fn due_hit_inside_window() {
        let entries = [e1()];
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 0, 0), &no_fired(), DEFAULT_WINDOW_MS),
            Some(&entries[0]),
            "窗口起点 ts 应命中"
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &no_fired(), DEFAULT_WINDOW_MS),
            Some(&entries[0])
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 4, 59), &no_fired(), DEFAULT_WINDOW_MS),
            Some(&entries[0]),
            "窗口终点前 1s 应命中"
        );
    }

    #[test]
    fn due_miss_outside_window() {
        let entries = [e1()];
        assert_eq!(
            resolve_due_entry(&entries, at(0, 8, 59, 59), &no_fired(), DEFAULT_WINDOW_MS),
            None,
            "ts 前 1s 不触发"
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 7, 0, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 5, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None,
            "ts+window 恰为右开边界，不触发"
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 6, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 23, 0, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
    }

    #[test]
    fn due_window_ms_adjustable_right_open() {
        let entries = [e1()];
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 0, 59), &no_fired(), 60_000),
            Some(&entries[0]),
            "1 分钟窗内(59s)命中"
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &no_fired(), 60_000),
            None,
            "ts+60s 右开边界不触发"
        );
    }

    #[test]
    fn due_pure_function_does_not_mutate_last_fired() {
        let entries = [e1()];
        let lf = no_fired();
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            Some(&entries[0])
        );
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            Some(&entries[0]),
            "未更新 last_fired 时重复调用仍应命中"
        );
        assert!(lf.is_empty(), "resolve_due_entry 不得修改入参 last_fired");
    }

    #[test]
    fn due_already_fired_not_triggered() {
        let entries = [e1()];
        let mut lf = no_fired();
        lf.insert("s1".into(), at(0, 9, 0, 0));
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            None,
            "上次触发时刻 == ts → 已触发"
        );
        lf.insert("s1".into(), at(0, 9, 0, 0) + 1);
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            None
        );
    }

    #[test]
    fn due_clock_rollback_safe() {
        let entries = [e1()];
        let mut lf = no_fired();
        lf.insert("s1".into(), at(0, 22, 0, 0));
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            None
        );
    }

    #[test]
    fn due_cross_day() {
        let entries = [e1()];
        let mut lf = no_fired();
        lf.insert("s1".into(), at(-1, 9, 0, 0));
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 1, 0), &lf, DEFAULT_WINDOW_MS),
            Some(&entries[0]),
            "昨日触发记录 < 今日 ts，今日应重新触发"
        );
    }

    #[test]
    fn due_multiple_picks_latest_ts() {
        let a = ScheduleEntry { id: "a".into(), time: "09:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        let b = ScheduleEntry { id: "b".into(), time: "09:03".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        let c = ScheduleEntry { id: "c".into(), time: "09:01".into(), mode_id: "x".into(), action: "ask".into(), enabled: true };
        let entries = [a, b, c];
        assert_eq!(
            resolve_due_entry(&entries, at(0, 9, 4, 0), &no_fired(), DEFAULT_WINDOW_MS),
            Some(&entries[1]),
            "09:03 > 09:01 > 09:00"
        );
        let entries2 = [entries[0].clone(), entries[2].clone()];
        assert_eq!(
            resolve_due_entry(&entries2, at(0, 9, 4, 0), &no_fired(), DEFAULT_WINDOW_MS),
            Some(&entries2[1])
        );
    }

    #[test]
    fn due_disabled_entry_skipped() {
        let off = ScheduleEntry { id: "s1".into(), time: "09:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: false };
        assert_eq!(
            resolve_due_entry(&[off], at(0, 9, 1, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
    }

    #[test]
    fn due_invalid_time_skipped() {
        let bad = ScheduleEntry { id: "x".into(), time: "9:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        assert_eq!(
            resolve_due_entry(&[bad], at(0, 9, 1, 0), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
        assert_eq!(
            resolve_due_entry(&[], now_10am(), &no_fired(), DEFAULT_WINDOW_MS),
            None
        );
    }

    // ------------------------------------------------------- align_startup

    #[test]
    fn align_auto_not_fired_supplements() {
        let e = e1();
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &no_fired()).map(|x| x.id.clone()), Some("s1".to_string()));
    }

    #[test]
    fn align_auto_fired_no_supplement() {
        let e = e1();
        let mut lf = HashMap::new();
        lf.insert("s1".into(), at(0, 9, 0, 0));
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &lf), None);
    }

    #[test]
    fn align_ask_missed_no_popup() {
        let e = ScheduleEntry { id: "s2".into(), time: "09:00".into(), mode_id: "x".into(), action: "ask".into(), enabled: true };
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &no_fired()), None);
    }

    #[test]
    fn align_none_passed_today() {
        let e = ScheduleEntry { id: "s1".into(), time: "22:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &no_fired()), None);
        assert_eq!(align_startup(&[], at(0, 10, 0, 0), &no_fired()), None);
    }

    #[test]
    fn align_latest_only_no_fallback() {
        let older = ScheduleEntry { id: "b".into(), time: "08:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        let newer_ask = ScheduleEntry { id: "a".into(), time: "09:00".into(), mode_id: "x".into(), action: "ask".into(), enabled: true };
        assert_eq!(
            align_startup(&[older.clone(), newer_ask], at(0, 10, 0, 0), &no_fired()),
            None,
            "最新（09:00）是 ask → None，不回退到更早的 auto"
        );
        let newer_auto = ScheduleEntry { id: "a".into(), time: "09:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        assert_eq!(
            align_startup(&[older, newer_auto], at(0, 10, 0, 0), &no_fired()).map(|x| x.id.clone()),
            Some("a".to_string())
        );
    }

    #[test]
    fn align_latest_fired_no_fallback() {
        let older = ScheduleEntry { id: "b".into(), time: "08:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        let newer = ScheduleEntry { id: "a".into(), time: "09:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        let mut lf = HashMap::new();
        lf.insert("a".into(), at(0, 9, 0, 0));
        assert_eq!(align_startup(&[older, newer], at(0, 10, 0, 0), &lf), None);
    }

    #[test]
    fn align_now_equals_ts_counts_as_passed() {
        let e = ScheduleEntry { id: "s1".into(), time: "10:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: true };
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &no_fired()).map(|x| x.id.clone()), Some("s1".to_string()));
    }

    #[test]
    fn align_disabled_skipped() {
        let e = ScheduleEntry { id: "s1".into(), time: "09:00".into(), mode_id: "x".into(), action: "auto".into(), enabled: false };
        assert_eq!(align_startup(&[e], at(0, 10, 0, 0), &no_fired()), None);
    }

    // ---------------------------------------------------------- mark_fired

    #[test]
    fn mark_fired_returns_new_map() {
        let mut orig = HashMap::new();
        orig.insert("keep".into(), 111i64);
        let out = mark_fired(&orig, "s1", now_10am());
        assert_eq!(out.get("keep"), Some(&111i64));
        assert_eq!(out.get("s1"), Some(&now_10am()));
        assert_eq!(orig.len(), 1, "入参不得被修改");
    }

    #[test]
    fn mark_fired_overwrites_same_id() {
        let mut orig = HashMap::new();
        orig.insert("s1".into(), at(-1, 9, 0, 0));
        let out = mark_fired(&orig, "s1", now_10am());
        assert_eq!(out.get("s1"), Some(&now_10am()));
    }

    #[test]
    fn mark_fired_empty_input_works() {
        let out = mark_fired(&HashMap::new(), "s1", now_10am());
        assert_eq!(out.get("s1"), Some(&now_10am()));
    }
}
