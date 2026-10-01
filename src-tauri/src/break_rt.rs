//! 休息提醒运行时壳——对照 index.js 的 initBreakSystem / handleBreakEvent / showBreakWindow
//!
//! 【v0.2.1 修复】窗口动作改为「幂等状态驱动」：每 tick 按当前状态直接决策
//! （Show / PushUpdate / Hide），不再依赖 before→after 变化检测——
//! 修复「跳过/推迟等命令直接改状态时，窗口永不关闭」的缺陷。

use crate::break_timer::BreakState;
use crate::state::SharedState;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

/// 窗口动作决策（对照 JS handleBreakEvent 的 switch——纯函数以便回归测试）
#[derive(Debug, PartialEq, Eq)]
pub enum WindowAction {
    Show,
    PushUpdate,
    Hide,
}

pub fn window_action_for(state: BreakState) -> WindowAction {
    match state {
        BreakState::Alerting => WindowAction::Show,
        BreakState::Resting => WindowAction::PushUpdate,
        BreakState::Working | BreakState::Paused | BreakState::Idle => WindowAction::Hide,
    }
}

/// 推送 break:update（对照 pushBreakUpdate）
pub fn push_break_update(app: &AppHandle) {
    let payload = {
        let state = app.state::<SharedState>();
        let st = state.lock().unwrap();
        let snap = st.break_timer.get_state(now_ms());
        json!({
            "state": snap.state.as_str(),
            "remainingSeconds": snap.remaining_seconds,
            "pausedUntil": snap.paused_until
        })
    };
    let _ = app.emit("break:update", payload);
}

/// 显示休息窗口（对照 showBreakWindow：gentle 420×320 居中 / fullscreen 全屏）；已存在则仅更新
pub fn show_break_window(app: &AppHandle) {
    if app.get_webview_window("break").is_some() {
        push_break_update(app);
        return;
    }
    let style = {
        let state = app.state::<SharedState>();
        let st = state.lock().unwrap();
        st.settings
            .get()
            .get("breaks")
            .and_then(|b| b.get("style"))
            .and_then(|v| v.as_str())
            .unwrap_or("gentle")
            .to_string()
    };

    let mut builder = WebviewWindowBuilder::new(app, "break", WebviewUrl::App("break.html".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .visible(false)
        .focused(false)
        .on_page_load(move |window, load_payload| {
            if matches!(load_payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let _ = window.emit("break:update", json!({"ping": true}));
            }
        });

    if style == "fullscreen" {
        if let Ok(Some(mon)) = app.primary_monitor() {
            let size = mon.size();
            let pos = mon.position();
            builder = builder
                .position(pos.x as f64, pos.y as f64)
                .inner_size(size.width as f64, size.height as f64);
        }
    } else {
        builder = builder.inner_size(420.0, 320.0).center();
    }

    match builder.build() {
        Ok(w) => {
            let _ = w.show(); // 不聚焦（showInactive 语义）
            eprintln!("[break] 提醒窗口已显示（{style}）");
        }
        Err(e) => eprintln!("[break] 建窗失败: {e}"),
    }
    push_break_update(app);
}

/// 隐藏休息窗口（对照 hideBreakWindow）；窗口不存在时为无操作（幂等）
pub fn hide_break_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("break") {
        let _ = w.close();
    }
}

/// 暂停提醒（对照 pauseBreaks）：pause + 持久化 pausedUntil + 隐藏窗口
pub fn pause_breaks(app: &AppHandle, hours: i64) -> bool {
    let now = now_ms();
    let until = now + hours * 3_600_000;
    let ok = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let ok = st.break_timer.pause(now, Some(until));
        if ok {
            let _ = st.settings.save(&json!({ "pausedUntil": until }));
        }
        ok
    };
    hide_break_window(app);
    ok
}

/// 恢复提醒（对照 resumeBreaks）
pub fn resume_breaks(app: &AppHandle) -> bool {
    let now = now_ms();
    let ok = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let ok = st.break_timer.resume(now);
        if ok {
            let _ = st.settings.save(&json!({ "pausedUntil": null }));
        }
        ok
    };
    ok
}

/// 启动休息系统（对照 initBreakSystem + 1s tick 循环）
pub fn start_break_system(app: AppHandle) {
    {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let b = st.settings.get().get("breaks").cloned().unwrap_or(json!({}));
        let work = b.get("workSeconds").and_then(|v| v.as_u64());
        let brk = b.get("breakSeconds").and_then(|v| v.as_u64());
        let now = now_ms();
        st.break_timer.configure(work, brk, now);

        let enabled = b.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        if enabled {
            st.break_timer.start(now);
            let paused_until = st.settings.get().get("pausedUntil").and_then(|v| v.as_i64());
            if let Some(pu) = paused_until {
                if pu > now {
                    st.break_timer.pause(now, Some(pu));
                    eprintln!("[break] 已按上次暂停状态恢复");
                }
            }
        }
        eprintln!(
            "[break] system initialized, enabled={enabled}, work={:?}s, break={:?}s",
            work, brk
        );
    }

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let now = now_ms();
        let state = {
            let state = app.state::<SharedState>();
            let mut st = state.lock().unwrap();
            st.break_timer.tick(now);
            st.break_timer.get_state(now).state
        };
        // 幂等窗口动作：每 tick 按当前状态直接决策（修复「命令变更漏检」根因，
        // 见 v0.2.1：跳过/推迟直接改状态时，旧的变化检测会漏掉窗口动作）
        match window_action_for(state) {
            WindowAction::Show => show_break_window(&app),
            WindowAction::PushUpdate => push_break_update(&app),
            WindowAction::Hide => hide_break_window(&app),
        }
    });
    // 注：睡眠唤醒对齐由 scheduler 侧负责；休息状态机为时间戳基准，tick 频率不影响正确性。
}

// ============================================================
// 回归测试——窗口动作映射契约（防「命令改状态却不动窗口」类缺陷回潮）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerting_shows_window() {
        assert_eq!(window_action_for(BreakState::Alerting), WindowAction::Show);
    }

    #[test]
    fn resting_keeps_window_and_updates() {
        assert_eq!(
            window_action_for(BreakState::Resting),
            WindowAction::PushUpdate
        );
    }

    #[test]
    fn working_paused_idle_hide_window() {
        assert_eq!(window_action_for(BreakState::Working), WindowAction::Hide);
        assert_eq!(window_action_for(BreakState::Paused), WindowAction::Hide);
        assert_eq!(window_action_for(BreakState::Idle), WindowAction::Hide);
    }
}
