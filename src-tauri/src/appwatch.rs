//! 色彩敏感应用监控（W3）——白名单应用在前台时自动挂起护眼滤镜
//!
//! 借鉴 LightBulb 的「色彩敏感应用白名单」：
//!   - 打开 PS / 剪辑软件等色彩敏感程序 → 自动恢复原色（不干扰专业色彩判断）
//!   - 切走（前台换成其他应用）→ 自动恢复护眼滤镜
//!
//! 设计要点：
//!   - **挂起只针对色温（gamma），不触碰亮度**——亮度（背光）不影响色彩准确性，
//!     且还原背光会打断用户的亮度设置
//!   - 挂起状态只在内存（`AppState.app_suspended`），**不写 settings**——
//!     用户配置的 `enabled` 保持不变，应用重启后以 settings 为准
//!   - 检测：Win32 前台窗口 → 线程 PID → 进程映像名（GetForegroundWindow 系列）
//!   - 轮询 2 秒间隔（够快且开销可忽略）；由独立线程驱动

use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::SharedState;

/// 轮询间隔
const POLL_INTERVAL: Duration = Duration::from_secs(2);

// ============================================================
// 纯逻辑（可测）
// ============================================================

/// 归一化进程名：取路径最后一段、去 `.exe` 后缀、转小写
///
/// `"C:\\Program Files\\Adobe\\Photoshop.exe"` → `"photoshop"`
pub fn normalize_process_name(raw: &str) -> String {
    let base = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
    let lower = base.trim().to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .unwrap_or(&lower)
        .to_string()
}

/// 白名单匹配：大小写不敏感、忽略 `.exe` 后缀差异、跳过空白项
pub fn is_whitelisted(process_name: &str, whitelist: &[String]) -> bool {
    let needle = normalize_process_name(process_name);
    if needle.is_empty() {
        return false;
    }
    whitelist
        .iter()
        .any(|w| normalize_process_name(w) == needle)
}

/// 单次轮询的动作决策（纯函数，便于回归测试）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchAction {
    /// 白名单应用进入前台 → 挂起滤镜
    Suspend,
    /// 白名单应用仍在前台且处于挂起 → 保持（幂等，确保屏幕处于还原态）
    Stay,
    /// 白名单应用已离开 → 恢复滤镜
    Resume,
    /// 无变化
    None,
}

pub fn watch_action(matched: bool, already_suspended: bool) -> WatchAction {
    match (matched, already_suspended) {
        (true, false) => WatchAction::Suspend,
        (true, true) => WatchAction::Stay,
        (false, true) => WatchAction::Resume,
        (false, false) => WatchAction::None,
    }
}

// ============================================================
// Win32：前台进程名
// ============================================================

/// 读前台窗口的进程完整映像路径（失败返回 None）
pub fn foreground_process_path() -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        res.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// 读前台进程名（归一化；失败返回 None）
pub fn foreground_process_name() -> Option<String> {
    foreground_process_path().map(|p| normalize_process_name(&p))
}

// ============================================================
// 运行时：轮询循环 + 挂起/恢复
// ============================================================

/// 读配置：`colorSensitive = { enabled, apps[] }`
fn read_config(app: &AppHandle) -> (bool, Vec<String>) {
    let state = app.state::<SharedState>();
    let st = state.lock().unwrap();
    let cs = st.settings.get().get("colorSensitive").cloned().unwrap_or_default();
    let enabled = cs.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let apps = cs
        .get("apps")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    (enabled, apps)
}

/// 单次轮询 tick：检测前台应用并执行挂起/恢复（导出供测试与调试）
pub fn tick(app: &AppHandle) {
    let (enabled, list) = read_config(app);

    // 配置关闭时：若处于挂起中，先恢复
    if !enabled {
        let was_suspended = {
            let state = app.state::<SharedState>();
            let st = state.lock().unwrap();
            st.app_suspended.is_some()
        };
        if was_suspended {
            resume_filter(app, "配置关闭");
        }
        return;
    }

    let fg_name = foreground_process_name();
    let matched = fg_name
        .as_deref()
        .map(|n| is_whitelisted(n, &list))
        .unwrap_or(false);

    let suspended = {
        let state = app.state::<SharedState>();
        let st = state.lock().unwrap();
        st.app_suspended.is_some()
    };

    match watch_action(matched, suspended) {
        WatchAction::Suspend => suspend_filter(app, fg_name.as_deref().unwrap_or("?")),
        WatchAction::Stay => {
            // 幂等：确保屏幕保持还原态（用户若手动改过，这里会再次还原——
            // 这是设计内行为：白名单应用在前台时滤镜保持挂起）
            let state = app.state::<SharedState>();
            let mut st = state.lock().unwrap();
            if st.display.enabled {
                st.display.enabled = false;
                let _ = st.display.restore();
            }
        }
        WatchAction::Resume => resume_filter(app, "应用已离开"),
        WatchAction::None => {}
    }
}

/// 挂起滤镜：恢复原色 + 内存禁用（不改 settings.enabled）
fn suspend_filter(app: &AppHandle, app_name: &str) {
        let payload = {
            let state = app.state::<SharedState>();
            let mut st = state.lock().unwrap();
            st.display.enabled = false;
            let _ = st.display.restore();
            st.app_suspended = Some(app_name.to_string());
            crate::state::display_payload(&st)
        };
    let _ = app.emit("display:changed", payload);
    eprintln!("[appwatch] 色彩敏感应用进入前台（{app_name}）→ 已挂起滤镜");
}

/// 恢复滤镜：重新应用当前色温 + 清除挂起标记
fn resume_filter(app: &AppHandle, reason: &str) {
    let (payload, temp) = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        st.app_suspended = None;
        // 仅当用户配置仍启用时才恢复滤镜
        let user_enabled = st
            .settings
            .get()
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if user_enabled {
            st.display.enabled = true;
            let temp = st.display.current_temperature;
            let _ = st.display.apply(temp);
        }
        (crate::state::display_payload(&st), st.display.current_temperature)
    };
    let _ = app.emit("display:changed", payload);
    eprintln!("[appwatch] 滤镜已恢复（{reason}，{temp}K）");
}

/// 启动监控循环（独立线程；2 秒轮询）
pub fn start_appwatch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(POLL_INTERVAL);
        tick(&app);
    });
    eprintln!("[appwatch] 色彩敏感应用监控已启动（2s 轮询）");
}

// ============================================================
// 测试
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn normalize_strips_path_and_exe() {
        assert_eq!(
            normalize_process_name("C:\\Program Files\\Adobe\\Photoshop.exe"),
            "photoshop"
        );
        assert_eq!(normalize_process_name("/usr/bin/foo"), "foo");
        assert_eq!(normalize_process_name("Photoshop.EXE"), "photoshop");
        assert_eq!(normalize_process_name("  Code.exe  "), "code");
        assert_eq!(normalize_process_name(""), "");
    }

    #[test]
    fn whitelist_match_case_insensitive_and_suffix_tolerant() {
        let wl = list(&["photoshop", "Premiere Pro.exe"]);
        // 白名单无后缀、检测值有后缀
        assert!(is_whitelisted("photoshop.exe", &wl));
        // 大小写混合
        assert!(is_whitelisted("PhotoShop.exe", &wl));
        // 白名单有后缀、检测值无后缀
        assert!(is_whitelisted("premiere pro", &wl));
        // 完整路径（归一化后取末段精确相等）
        assert!(is_whitelisted("C:\\Program Files\\Adobe\\Photoshop.exe", &wl));
        // 精确匹配语义：不做部分词匹配（"adobe premiere pro" ≠ "premiere pro"）
        assert!(!is_whitelisted(
            "C:\\Program Files\\Adobe\\Adobe Premiere Pro.exe",
            &wl
        ));
        // 不在名单
        assert!(!is_whitelisted("chrome", &wl));
    }

    #[test]
    fn whitelist_skips_blank_and_handles_empty_inputs() {
        let wl = list(&["", "   ", "code"]);
        assert!(is_whitelisted("code.exe", &wl));
        assert!(!is_whitelisted("", &wl));
        assert!(!is_whitelisted("anything", &list(&[])));
        // 全空白输入不匹配任何项（含空项白名单）
        assert!(!is_whitelisted("   ", &wl));
    }

    #[test]
    fn watch_action_state_machine() {
        use WatchAction::*;
        assert_eq!(watch_action(true, false), Suspend);
        assert_eq!(watch_action(true, true), Stay);
        assert_eq!(watch_action(false, true), Resume);
        assert_eq!(watch_action(false, false), None);
    }
}
