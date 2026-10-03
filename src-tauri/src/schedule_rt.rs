//! 时间调度运行时壳——对照 index.js 的 checkSchedule / handleScheduleEntry / alignScheduleOnBoot
//!
//! 30s tick + 跳变检测（距上次 tick 超 90s 视为睡眠唤醒 → 对齐）。

use crate::scheduler::{align_startup, mark_fired, resolve_due_entry, ScheduleEntry, DEFAULT_WINDOW_MS};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// 从 settings 读出 (enabled, entries, last_fired)
fn read_schedule(app: &AppHandle) -> Option<(Vec<ScheduleEntry>, HashMap<String, i64>)> {
    let state = app.state::<Mutex<crate::state::AppState>>();
    let st = state.lock().unwrap();
    let sch = st.settings.get().get("schedule")?;
    if !sch.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    let entries: Vec<ScheduleEntry> =
        serde_json::from_value(sch.get("entries").cloned().unwrap_or(json!([]))).ok()?;
    let fired: HashMap<String, i64> = sch
        .get("lastFired")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Some((entries, fired))
}

fn save_fired(app: &AppHandle, next_fired: HashMap<String, i64>) {
    let state = app.state::<Mutex<crate::state::AppState>>();
    let mut st = state.lock().unwrap();
    let _ = st
        .settings
        .save(&json!({ "schedule": { "lastFired": next_fired } }));
}

/// 处理一条到点条目（对照 handleScheduleEntry）
fn handle_schedule_entry(app: &AppHandle, entry: &ScheduleEntry, now: i64) {
    let Some(mode) = crate::modes::get_mode(&entry.mode_id) else {
        return;
    };
    // 触发即持久化 lastFired（本窗口消费，防重跨重启）
    let fired = read_schedule(app).map(|(_, f)| f).unwrap_or_default();
    let next_fired = mark_fired(&fired, &entry.id, now);
    save_fired(app, next_fired);

    if entry.action == "ask" {
        crate::propose::show_propose(
            app,
            json!({
                "kind": "schedule",
                "modeId": entry.mode_id,
                "title": format!("现在是 {}", entry.time),
                "body": format!("切换到「{} {}K」吗？", mode.name, mode.kelvin)
            }),
        );
    } else {
        let r = crate::commands::apply_mode_with_app(app, &entry.mode_id, true, false);
        eprintln!(
            "[schedule] auto → {} ok={}",
            entry.mode_id,
            r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
        );
        // 托盘气泡（notifyBalloon）——W4 托盘接入后补齐
    }
}

/// 单次调度检查（对照 checkSchedule）
pub fn check_schedule_once(app: &AppHandle, now: i64) {
    let Some((entries, fired)) = read_schedule(app) else {
        return;
    };
    if let Some(entry) = resolve_due_entry(&entries, now, &fired, DEFAULT_WINDOW_MS) {
        let entry = entry.clone();
        handle_schedule_entry(app, &entry, now);
    }
}

/// 启动 / 唤醒对齐（对照 alignScheduleOnBoot）：今日已过最新 auto 未触发 → 静默应用
pub fn align_schedule_on_boot(app: &AppHandle, reason: &str) {
    let Some((entries, fired)) = read_schedule(app) else {
        return;
    };
    let now = chrono::Local::now().timestamp_millis();
    let Some(entry) = align_startup(&entries, now, &fired) else {
        return;
    };
    if crate::modes::get_mode(&entry.mode_id).is_none() {
        return;
    }
    let next_fired = mark_fired(&fired, &entry.id, now);
    save_fired(app, next_fired);
    crate::commands::apply_mode_with_app(app, &entry.mode_id, true, true);
    eprintln!("[schedule] 对齐（{reason}）：{} → {}", entry.time, entry.mode_id);
}

/// 启动 30s tick 循环（含唤醒跳变检测）
pub fn start_scheduler(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(30));
            let now_instant = Instant::now();
            let jumped = now_instant.duration_since(last_tick) > Duration::from_secs(90);
            last_tick = now_instant;
            if jumped {
                // 睡眠唤醒：先对齐（错过窗口的 auto 补应用）
                align_schedule_on_boot(&app, "resume");
            }
            let now = chrono::Local::now().timestamp_millis();
            check_schedule_once(&app, now);
            // 日落跟随过渡检查（共用同一 tick；jumped=睡眠唤醒时走快速补过渡）
            crate::sun_rt::check_sun_follow(&app, now, jumped);
            // 用眼统计采样：睡眠唤醒（jumped）不补记，避免把睡眠时长算成用眼
            crate::usage::sample(&app, if jumped { 0 } else { 30 });
        }
    });
    eprintln!("[schedule] tick 已启动（30s）");
}
