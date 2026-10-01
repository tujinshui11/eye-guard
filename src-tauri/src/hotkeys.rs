//! 全局热键——对照移植 `src/main/hotkeys.js`（Ctrl+Alt+↑/↓ 色温 ±200K）
//!
//! 偏离记录：JS 版从 settings.hotkeys 读取键位字符串；Rust 版先以默认键位注册
//! （插件字符串格式与 JS 格式不同——自定义键位映射留待后续）。
//! v0.2.1：注册失败自动重试（覆盖快速重启/短暂占用的时序竞争场景）；失败不阻塞启动。

use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const KEYS: [(&str, f64); 2] = [("ctrl+alt+arrowup", 200.0), ("ctrl+alt+arrowdown", -200.0)];
const MAX_ATTEMPTS: u32 = 4;

pub fn register_hotkeys(app: &AppHandle) {
    let app2 = app.clone();
    std::thread::spawn(move || {
        let mut pending: Vec<(&str, f64)> = KEYS.to_vec();
        for attempt in 1..=MAX_ATTEMPTS {
            let mut still_failed: Vec<(&str, f64)> = Vec::new();
            for (key, delta) in pending.drain(..) {
                let a = app2.clone();
                let result = app2.global_shortcut().on_shortcut(key, move |_app, _sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        crate::commands::nudge_temperature_with_app(&a, delta);
                    }
                });
                match result {
                    Ok(_) => {
                        eprintln!("[hotkeys] {key} 注册成功（尝试 {attempt}/{MAX_ATTEMPTS}）")
                    }
                    Err(e) => {
                        if attempt == MAX_ATTEMPTS {
                            eprintln!(
                                "[hotkeys] 注册失败 {key}（已重试 {MAX_ATTEMPTS} 次）: {e}"
                            );
                        }
                        still_failed.push((key, delta));
                    }
                }
            }
            if still_failed.is_empty() {
                return;
            }
            pending = still_failed;
            if attempt < MAX_ATTEMPTS {
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    });
}
