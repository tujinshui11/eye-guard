pub mod modes;
pub mod settings;
pub mod temperature;

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
        .setup(|app| {
            // --hidden 启动（开机自启场景）不显示主窗口；正常启动显示
            let hidden = std::env::args().any(|a| a == "--hidden");
            if let Some(w) = app.get_webview_window("main") {
                if !hidden {
                    let _ = w.show();
                }
            }
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
