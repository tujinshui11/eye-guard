//! 亮度遮罩（overlay）——对照移植 `src/main/overlay.js`
//!
//! 透明全屏窗口 + 点击穿透 + 置顶；亮度经 rgba 遮罩 alpha 实现（不占用 gamma 空间）。
//! v0.2.2 修复：Tauri 的 eval 不等待页面加载（与 Electron executeJavaScript 语义不同）——
//! 首次建窗时直接 eval 会丢；新增 on_page_load(Finished) 兜底应用当前亮度。

use crate::state::SharedState;
use serde_json::json;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

fn alpha_for(v: u32) -> f64 {
    (100 - v) as f64 / 100.0
}

/// 对遮罩窗口应用 alpha（页面未就绪时的调用会被 on_page_load 兜底覆盖）
fn apply_mask(window: &tauri::WebviewWindow, alpha: f64) {
    let _ = window.eval(format!("window.setMaskAlpha({alpha})"));
}

/// 确保遮罩窗口存在（对照 ensureWindow：全屏 + 穿透 + 置顶 + 不聚焦）
fn ensure_overlay_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window("overlay") {
        return Some(w);
    }
    let (mut x, mut y, mut width, mut height) = (0.0f64, 0.0f64, 1920.0f64, 1080.0f64);
    if let Ok(Some(mon)) = app.primary_monitor() {
        let size = mon.size();
        let pos = mon.position();
        x = pos.x as f64;
        y = pos.y as f64;
        width = size.width as f64;
        height = size.height as f64;
    }
    let app_for_load = app.clone();
    match WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false)
        .inner_size(width, height)
        .position(x, y)
        .on_page_load(move |window, load_payload| {
            // v0.2.2：页面就绪后兜底应用当前亮度（首次创建时 eval 可能落在未加载文档上）
            if matches!(load_payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let current = {
                    let state = app_for_load.state::<SharedState>();
                    let st = state.lock().unwrap();
                    st.overlay_brightness
                };
                let alpha = alpha_for(current);
                if alpha > 0.001 {
                    apply_mask(&window, alpha);
                }
            }
        })
        .build()
    {
        Ok(w) => {
            // 点击穿透（对照 setIgnoreMouseEvents(true, {forward:true})）
            let _ = w.set_ignore_cursor_events(true);
            Some(w)
        }
        Err(e) => {
            eprintln!("[overlay] 建窗失败: {e}");
            None
        }
    }
}

/// 设置亮度（对照 overlay.setBrightness）：clamp 50–100；alpha=(100-b)/100；
/// alpha≤0.001 → 隐藏；否则 eval setMaskAlpha + show
pub fn set_overlay_brightness(app: &AppHandle, brightness: f64) -> u32 {
    let v = brightness.clamp(50.0, 100.0) as u32;
    let alpha = alpha_for(v);

    {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        st.overlay_brightness = v;
    }

    if alpha <= 0.001 {
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide();
        }
        return v;
    }

    if let Some(w) = ensure_overlay_window(app) {
        apply_mask(&w, alpha);
        let _ = w.show();
    }
    v
}

/// 恢复/隐藏遮罩（供 restore 场景调用——遮罩回到 100%（隐藏））
pub fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
    let state = app.state::<SharedState>();
    let mut st = state.lock().unwrap();
    st.overlay_brightness = 100;
}

/// 供 display:changed 载荷的 brightness 值（overlay 接入后）
pub fn current_brightness(app: &AppHandle) -> u32 {
    let state = app.state::<SharedState>();
    let st = state.lock().unwrap();
    st.overlay_brightness
}

/// 供调试：状态 JSON
#[allow(dead_code)]
pub fn overlay_debug_state(app: &AppHandle) -> serde_json::Value {
    json!({ "brightness": current_brightness(app) })
}
