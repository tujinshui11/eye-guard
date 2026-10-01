//! 托盘——对照移植 `src/main/tray.js`（7 档模式 check 组 / 微调 / 恢复 / 暂停 / 设置 / 退出；
//! 动作后重建整菜单）

use crate::break_timer::BreakState;
use crate::state::SharedState;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

/// 构建菜单（每次动作后重建——对照 tray.js 的 rebuild 机制）
fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let (enabled, mode_id, paused) = {
        let state = app.state::<SharedState>();
        let st = state.lock().unwrap();
        let s = st.settings.get();
        let enabled = s.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let mode_id = s
            .get("modeId")
            .and_then(|v| v.as_str())
            .unwrap_or("natural")
            .to_string();
        let paused = st.break_timer.get_state(now_ms()).state == BreakState::Paused;
        (enabled, mode_id, paused)
    };

    let title = MenuItemBuilder::with_id("title", "护眼助手")
        .enabled(false)
        .build(app)?;
    let mut builder = MenuBuilder::new(app).item(&title).separator();

    for m in crate::modes::MODES.iter() {
        let checked = enabled && mode_id == m.id;
        let item = CheckMenuItemBuilder::with_id(format!("mode:{}", m.id), format!("{}（{}K）", m.name, m.kelvin))
            .checked(checked)
            .build(app)?;
        builder = builder.item(&item);
    }

    let temp_up = MenuItemBuilder::with_id("temp_up", "色温 +200K").build(app)?;
    let temp_down = MenuItemBuilder::with_id("temp_down", "色温 −200K").build(app)?;
    let restore = if enabled {
        MenuItemBuilder::with_id("restore", "恢复原色").build(app)?
    } else {
        MenuItemBuilder::with_id("reenable", "重新启用").build(app)?
    };
    let pause = if paused {
        MenuItemBuilder::with_id("resume_breaks", "恢复提醒").build(app)?
    } else {
        MenuItemBuilder::with_id("pause_breaks", "暂停提醒 1 小时").build(app)?
    };
    let settings_item = MenuItemBuilder::with_id("settings", "设置…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    builder
        .item(&temp_up)
        .item(&temp_down)
        .separator()
        .item(&restore)
        .separator()
        .item(&pause)
        .separator()
        .item(&settings_item)
        .item(&quit)
        .build()
}

/// 重建菜单（动作后调用）
fn rebuild(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    if let Some(mode_id) = id.strip_prefix("mode:") {
        crate::commands::apply_mode_with_app(app, mode_id, true, false);
    } else {
        match id {
            "temp_up" => crate::commands::nudge_temperature_with_app(app, 200.0),
            "temp_down" => crate::commands::nudge_temperature_with_app(app, -200.0),
            "restore" => {
                crate::commands::restore_color_with_app(app);
            }
            "reenable" => crate::commands::reenable_color_with_app(app),
            "pause_breaks" => {
                crate::break_rt::pause_breaks(app, 1);
            }
            "resume_breaks" => {
                crate::break_rt::resume_breaks(app);
            }
            "settings" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        }
    }
    rebuild(app);
}

/// 创建托盘（图标取自 bundle 默认图标）
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_tray_icon_event(|tray, event| {
            // 左键抬起 → 切换主窗口显隐（对照 tray.on('click', toggleWindow)）
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    eprintln!("[tray] created");
    Ok(())
}
