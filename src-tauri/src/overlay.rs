//! 亮度遮罩（overlay）——对照移植 `src/main/overlay.js`
//!
//! 透明全屏窗口 + 点击穿透 + 置顶；亮度经 rgba 遮罩 alpha 实现（不占用 gamma 空间）。

use crate::state::SharedState;
use serde_json::json;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

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
    let alpha = (100 - v) as f64 / 100.0;

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
        let _ = w.eval(format!("window.setMaskAlpha({alpha})"));
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
