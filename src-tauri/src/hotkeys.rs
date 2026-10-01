//! 全局热键——对照移植 `src/main/hotkeys.js`（Ctrl+Alt+↑/↓ 色温 ±200K）
//!
//! 偏离记录：JS 版从 settings.hotkeys 读取键位字符串；Rust 版先以默认键位注册
//! （插件字符串格式与 JS 格式不同——自定义键位映射留待后续）。

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn register_hotkeys(app: &AppHandle) {
    let gs = app.global_shortcut();
    for (key, delta) in [("ctrl+alt+arrowup", 200.0f64), ("ctrl+alt+arrowdown", -200.0)] {
        let app2 = app.clone();
        let result = gs.on_shortcut(key, move |_app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                crate::commands::nudge_temperature_with_app(&app2, delta);
            }
        });
        if let Err(e) = result {
            eprintln!("[hotkeys] 注册失败 {key}: {e}");
        }
    }
    eprintln!("[hotkeys] registered (ctrl+alt+arrowup/down)");
}
