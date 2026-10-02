//! 护眼助手 Rust/Tauri 2 重写——应用装配
//! 对照 Electron 版 src/main/index.js 的生命周期/装配职责

pub mod ambient;
pub mod break_rt;
pub mod break_timer;
pub mod brightness;
pub mod commands;
pub mod display;
pub mod gamma;
pub mod hotkeys;
pub mod modes;
pub mod overlay;
pub mod presence;
pub mod propose;
pub mod schedule_rt;
pub mod scheduler;
pub mod settings;
pub mod state;
pub mod temperature;
pub mod tray;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二实例启动：唤起既有主窗口（对照 Electron 版 requestSingleInstanceLock 行为）
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::app_get_version,
            commands::display_get_state,
            commands::display_set_temperature,
            commands::display_set_brightness,
            commands::display_restore,
            commands::modes_list,
            commands::modes_apply,
            commands::settings_get,
            commands::settings_set,
            commands::break_get_state,
            commands::break_action,
            commands::app_set_auto_launch,
            commands::app_get_auto_launch,
            commands::propose_action,
            commands::ambient_get_state,
        ])
        .setup(|app| {
            // 全局状态：设置 + display（含 dirty 自愈与启动状态 silent 恢复）
            app.manage(std::sync::Mutex::new(state::bootstrap()));
            let handle = app.handle().clone();

            // 启动状态：亮度恢复（对照 applyStartupState 的 brightness < 100 → setBrightness 分支）
            // C 方案：走亮度控制器（背光主控 + 黑纱补充），而非旧遮罩直换
            let init_brightness = {
                let state = handle.state::<state::SharedState>();
                let st = state.lock().unwrap();
                st.settings
                    .get()
                    .get("brightness")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100)
            };
            {
                let state = handle.state::<state::SharedState>();
                let mut st = state.lock().unwrap();
                let (_outcome, alpha) = st.brightness.apply(init_brightness as u32);
                st.overlay_brightness = init_brightness as u32;
                drop(st);
                if alpha > 0.001 {
                    overlay::apply_mask_alpha(&handle, alpha);
                }
            }

            // 调度：启动对齐 + 30s tick（含唤醒跳变检测）
            schedule_rt::align_schedule_on_boot(&handle, "boot");
            schedule_rt::start_scheduler(handle.clone());

            // 感光：按配置启动（默认关闭）
            ambient::monitor::start_ambient_monitor(&handle, "boot");

            // 休息提醒系统（1s tick + 窗口）
            break_rt::start_break_system(handle.clone());

            // 托盘 + 全局热键
            if let Err(e) = tray::create_tray(&handle) {
                eprintln!("[boot] tray 创建失败: {e}");
            }
            hotkeys::register_hotkeys(&handle);

            // --hidden 启动（开机自启场景）不显示主窗口；正常启动显示
            let hidden = std::env::args().any(|a| a == "--hidden");
            if let Some(w) = app.get_webview_window("main") {
                if !hidden {
                    let _ = w.show();
                }
            }
            eprintln!("[boot] all initialized");
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭主窗口 = 隐藏到托盘（常驻），而非退出应用
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 退出前恢复原始色彩（README 承诺「退出自动还原屏幕原色」）
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app.state::<state::SharedState>();
                let mut st = state.lock().unwrap();
                let _ = st.display.restore();
                eprintln!("[exit] 已恢复原始色彩");
            }
        });
}
