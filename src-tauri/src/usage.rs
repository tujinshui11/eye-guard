//! 用眼统计——按日累计护眼时长、休息完成/跳过、24 小时分布
//!
//! 存储：`%APPDATA%\护眼助手\usage.json`（独立于 settings.json，避免设置读写互相干扰）
//! 保留：最近 60 天（自动裁剪）
//!
//! 采集口径：
//! - `active_seconds`：护眼生效（未挂起）的前台使用秒数——每 30s tick 累加
//! - `hourly[24]`：当天各时段的活跃秒数分布
//! - `breaks_completed` / `breaks_skipped`：休息流程的完成与跳过计数

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 保留天数
const KEEP_DAYS: usize = 60;

/// 单日统计
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DayStats {
    pub active_seconds: u64,
    pub breaks_completed: u32,
    pub breaks_skipped: u32,
    /// 24 小时分布（每小时活跃秒数）
    pub hourly: [u64; 24],
}

impl DayStats {
    fn from_json(v: &Value) -> Self {
        let mut hourly = [0u64; 24];
        if let Some(arr) = v.get("hourly").and_then(|x| x.as_array()) {
            for (i, item) in arr.iter().take(24).enumerate() {
                hourly[i] = item.as_u64().unwrap_or(0);
            }
        }
        Self {
            active_seconds: v.get("activeSeconds").and_then(|x| x.as_u64()).unwrap_or(0),
            breaks_completed: v
                .get("breaksCompleted")
                .and_then(|x| x.as_u64())
                .unwrap_or(0) as u32,
            breaks_skipped: v.get("breaksSkipped").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            hourly,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "activeSeconds": self.active_seconds,
            "breaksCompleted": self.breaks_completed,
            "breaksSkipped": self.breaks_skipped,
            "hourly": self.hourly.to_vec()
        })
    }

    /// 累计活跃秒数（含时段分布）
    pub fn add_active(&mut self, hour: usize, seconds: u64) {
        self.active_seconds = self.active_seconds.saturating_add(seconds);
        if hour < 24 {
            self.hourly[hour] = self.hourly[hour].saturating_add(seconds);
        }
    }
}

/// 统计库（日期 → 单日）
#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    pub days: BTreeMap<String, DayStats>,
}

impl UsageStats {
    pub fn load(dir: &Path) -> Self {
        let path = Self::file_path(dir);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            return Self::default();
        };
        let mut days = BTreeMap::new();
        if let Some(obj) = v.get("days").and_then(|x| x.as_object()) {
            for (date, day) in obj {
                days.insert(date.clone(), DayStats::from_json(day));
            }
        }
        Self { days }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let mut obj = serde_json::Map::new();
        for (date, day) in &self.days {
            obj.insert(date.clone(), day.to_json());
        }
        let payload = json!({ "days": obj });
        std::fs::write(Self::file_path(dir), serde_json::to_string_pretty(&payload).unwrap())
    }

    fn file_path(dir: &Path) -> PathBuf {
        dir.join("usage.json")
    }

    /// 取某日（不存在则创建空记录）
    pub fn day_mut(&mut self, date: &str) -> &mut DayStats {
        self.days.entry(date.to_string()).or_default()
    }

    /// 裁剪：只保留最近 KEEP_DAYS 天（BTreeMap 按日期字典序 = 时间序）
    pub fn prune(&mut self) {
        while self.days.len() > KEEP_DAYS {
            let first = self.days.keys().next().cloned();
            if let Some(k) = first {
                self.days.remove(&k);
            } else {
                break;
            }
        }
    }

    /// 概览：今日 + 近 7 日汇总 + 序列（供前端图表）
    pub fn summary_json(&self, today: &str) -> Value {
        let today_stats = self.days.get(today).cloned().unwrap_or_default();
        // 近 7 日（含今日）序列，缺失日补零
        let recent: Vec<(String, DayStats)> = self
            .days
            .iter()
            .rev()
            .take(7)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let week_active: u64 = recent.iter().map(|(_, d)| d.active_seconds).sum();
        let week_completed: u32 = recent.iter().map(|(_, d)| d.breaks_completed).sum();
        let week_skipped: u32 = recent.iter().map(|(_, d)| d.breaks_skipped).sum();

        json!({
            "today": {
                "date": today,
                "activeSeconds": today_stats.active_seconds,
                "breaksCompleted": today_stats.breaks_completed,
                "breaksSkipped": today_stats.breaks_skipped,
                "hourly": today_stats.hourly.to_vec()
            },
            "week": {
                "activeSeconds": week_active,
                "breaksCompleted": week_completed,
                "breaksSkipped": week_skipped,
                "days": recent.iter().map(|(d, s)| json!({
                    "date": d,
                    "activeSeconds": s.active_seconds,
                    "breaksCompleted": s.breaks_completed,
                    "breaksSkipped": s.breaks_skipped
                })).collect::<Vec<_>>()
            }
        })
    }
}

/// 当前日期（本地时区，YYYY-MM-DD）
pub fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// 当前小时（0-23，本地时区）
pub fn current_hour() -> usize {
    use chrono::Timelike;
    chrono::Local::now().hour() as usize
}

/// 采样一次（由 30s tick 调用）
///
/// `active_seconds` 为本次采样覆盖的活跃秒数（护眼生效时为 tick 间隔，否则 0）。
pub fn sample(app: &tauri::AppHandle, active_seconds: u64) {
    use tauri::Manager;
    let (dir, today) = (crate::state::data_dir(), today_key());
    let hour = current_hour();
    let state = app.state::<std::sync::Mutex<crate::state::AppState>>();
    let payload_json = {
        let mut st = state.lock().unwrap();
        if active_seconds > 0 {
            st.usage.day_mut(&today).add_active(hour, active_seconds);
        }
        st.usage.prune();
        // 落盘：仅在活跃时写（避免空转 I/O）
        if active_seconds > 0 {
            let _ = st.usage.save(&dir);
        }
        st.usage.summary_json(&today)
    };
    let _ = payload_json;
}

/// 记录一次休息完成
pub fn record_break_completed(app: &tauri::AppHandle) {
    record_break(app, true);
}

/// 记录一次休息跳过
pub fn record_break_skipped(app: &tauri::AppHandle) {
    record_break(app, false);
}

fn record_break(app: &tauri::AppHandle, completed: bool) {
    use tauri::Manager;
    let (dir, today) = (crate::state::data_dir(), today_key());
    let state = app.state::<std::sync::Mutex<crate::state::AppState>>();
    let mut st = state.lock().unwrap();
    let day = st.usage.day_mut(&today);
    if completed {
        day.breaks_completed = day.breaks_completed.saturating_add(1);
    } else {
        day.breaks_skipped = day.breaks_skipped.saturating_add(1);
    }
    let _ = st.usage.save(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_active_accumulates_total_and_hourly() {
        let mut d = DayStats::default();
        d.add_active(9, 30);
        d.add_active(9, 30);
        d.add_active(14, 60);
        assert_eq!(d.active_seconds, 120);
        assert_eq!(d.hourly[9], 60);
        assert_eq!(d.hourly[14], 60);
        assert_eq!(d.hourly[10], 0);
    }

    #[test]
    fn add_active_ignores_out_of_range_hour() {
        let mut d = DayStats::default();
        d.add_active(99, 30);
        assert_eq!(d.active_seconds, 30, "总数仍应累计");
        assert!(d.hourly.iter().all(|&x| x == 0), "越界小时不写入分布");
    }

    #[test]
    fn json_roundtrip_preserves_fields() {
        let mut d = DayStats::default();
        d.add_active(8, 120);
        d.breaks_completed = 3;
        d.breaks_skipped = 1;
        let back = DayStats::from_json(&d.to_json());
        assert_eq!(back, d);
    }

    #[test]
    fn from_json_tolerates_missing_and_short_hourly() {
        let v = json!({ "activeSeconds": 42, "hourly": [1, 2, 3] });
        let d = DayStats::from_json(&v);
        assert_eq!(d.active_seconds, 42);
        assert_eq!(d.hourly[0], 1);
        assert_eq!(d.hourly[3], 0, "缺失时段补零");
        assert_eq!(d.breaks_completed, 0);
    }

    #[test]
    fn prune_keeps_latest_n_days() {
        let mut u = UsageStats::default();
        for i in 1..=(KEEP_DAYS + 5) {
            u.day_mut(&format!("2026-01-{i:02}")).add_active(0, 1);
        }
        assert_eq!(u.days.len(), KEEP_DAYS + 5);
        u.prune();
        assert_eq!(u.days.len(), KEEP_DAYS);
        // 最早 5 天被裁掉
        assert!(!u.days.contains_key("2026-01-01"));
        assert!(u.days.contains_key(&format!("2026-01-{:02}", KEEP_DAYS + 5)));
    }

    #[test]
    fn summary_reports_today_and_week() {
        let mut u = UsageStats::default();
        u.day_mut("2026-10-01").add_active(10, 3600);
        u.day_mut("2026-10-02").add_active(11, 1800);
        let t = u.day_mut("2026-10-03");
        t.add_active(9, 600);
        t.breaks_completed = 2;
        let s = u.summary_json("2026-10-03");
        assert_eq!(s["today"]["activeSeconds"], 600);
        assert_eq!(s["today"]["breaksCompleted"], 2);
        assert_eq!(s["week"]["activeSeconds"], 3600 + 1800 + 600);
        assert_eq!(s["week"]["breaksCompleted"], 2);
        assert_eq!(s["week"]["days"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn summary_empty_stats_is_zeroed_not_missing() {
        let u = UsageStats::default();
        let s = u.summary_json("2026-10-03");
        assert_eq!(s["today"]["activeSeconds"], 0);
        assert_eq!(s["week"]["days"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("eyeguard-usage-test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut u = UsageStats::default();
        u.day_mut("2026-10-03").add_active(15, 900);
        u.day_mut("2026-10-03").breaks_completed = 4;
        u.save(&dir).expect("save 应成功");
        let back = UsageStats::load(&dir);
        assert_eq!(back.days.len(), 1);
        let d = back.days.get("2026-10-03").expect("应含该日");
        assert_eq!(d.active_seconds, 900);
        assert_eq!(d.breaks_completed, 4);
        assert_eq!(d.hourly[15], 900);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_or_broken_file_yields_empty() {
        let dir = std::env::temp_dir().join("eyeguard-usage-broken");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(UsageStats::load(&dir).days.is_empty(), "文件不存在应为空");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("usage.json"), "{ not json").unwrap();
        assert!(UsageStats::load(&dir).days.is_empty(), "损坏 json 应为空");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
