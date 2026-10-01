//! 建议卡片窗口（调度询问 / 感光提醒共用）——对照 index.js:393-460 的 show/resolvePropose

use crate::state::SharedState;
use serde_json::Value;
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// 显示建议卡片（建窗或复用 + 发送 payload + 60s 超时）
pub fn show_propose(app: &AppHandle, payload: Value) {
    let state = app.state::<SharedState>();

    // 记录 payload + 世代（防重）
    let generation = {
        let mut st = state.lock().unwrap();
        st.propose.generation += 1;
        st.propose.payload = Some(payload.clone());
        st.propose.generation
    };

    let send_payload = {
        let p = payload.clone();
        move |window: &tauri::WebviewWindow| {
            let _ = window.emit("propose:update", p.clone());
        }
    };

    match app.get_webview_window("propose") {
        Some(w) => {
            // 复用既有窗口：先发事件再显示（不抢焦点）
            send_payload(&w);
            let _ = w.show();
        }
        None => {
            let p = payload.clone();
            let builder = WebviewWindowBuilder::new(app, "propose", WebviewUrl::App("propose.html".into()))
                .title("")
                .inner_size(400.0, 190.0)
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
                    // 等页面加载完再发 payload（对照 did-finish-load）
                    if matches!(load_payload.event(), PageLoadEvent::Finished) {
                        let _ = window.emit("propose:update", p.clone());
                    }
                });
            match builder.build() {
                Ok(w) => {
                    let _ = w.show();
                }
                Err(e) => eprintln!("[propose] 建窗失败: {e}"),
            }
        }
    }

    // 60 秒无操作自动关闭（视为忽略）
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        let state = app2.state::<SharedState>();
        let still_current = {
            let st = state.lock().unwrap();
            st.propose.generation == generation && st.propose.payload.is_some()
        };
        if still_current {
            let _ = resolve_propose(&app2, "timeout");
        }
    });

    let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
    eprintln!("[propose] shown: {title}");
}

/// 卡片交互结果：apply=切换（payload.modeId）；其余（dismiss/mute/timeout）不应用。
/// 窗口已消费即防重。
pub fn resolve_propose(app: &AppHandle, action: &str) -> bool {
    let state = app.state::<SharedState>();
    let payload = {
        let mut st = state.lock().unwrap();
        st.propose.generation += 1; // 使在途超时失效
        st.propose.payload.take()
    };

    if let Some(w) = app.get_webview_window("propose") {
        let _ = w.close();
    }

    if action == "apply" {
        if let Some(mode_id) = payload
            .as_ref()
            .and_then(|p| p.get("modeId"))
            .and_then(|v| v.as_str())
        {
            let mode_id = mode_id.to_string();
            let app2 = app.clone();
            crate::commands::apply_mode_with_app(&app2, &mode_id, true, false);
            eprintln!("[propose] apply {mode_id}");
            return true;
        }
    }
    eprintln!("[propose] resolved: {action}");
    true
}
