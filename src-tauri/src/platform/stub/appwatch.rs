//! appwatch（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：`NSWorkspace.shared.frontmostApplication`（objc2）取前台应用
//!
//! 当前为占位：前台进程恒为 None → 白名单挂起逻辑静默不触发（不误挂起），
//! 纯逻辑函数（normalize/is_whitelisted/watch_action）为平台无关实现，与 Windows 版一致。

use tauri::AppHandle;

/// 单次轮询的动作决策（与 Windows 版一致，纯逻辑，平台无关）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchAction {
    Suspend,
    Stay,
    Resume,
    None,
}

/// 进程名归一化（平台无关：取末段、去 .exe 后缀、小写）——与 Windows 版逐行对齐
pub fn normalize_process_name(raw: &str) -> String {
    let base = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
    let lower = base.trim().to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// 是否在白名单内（走 normalize 比较，与 Windows 版语义一致：忽略 .exe 差异与大小写）
pub fn is_whitelisted(process_name: &str, whitelist: &[String]) -> bool {
    let needle = normalize_process_name(process_name);
    if needle.is_empty() {
        return false;
    }
    whitelist
        .iter()
        .any(|w| normalize_process_name(w) == needle)
}

/// 动作决策（平台无关）
pub fn watch_action(matched: bool, already_suspended: bool) -> WatchAction {
    match (matched, already_suspended) {
        (true, false) => WatchAction::Suspend,
        (true, true) => WatchAction::Stay,
        (false, true) => WatchAction::Resume,
        (false, false) => WatchAction::None,
    }
}

/// 前台进程路径（占位：不支持）
pub fn foreground_process_path() -> Option<String> {
    None
}

/// 前台进程名（占位：不支持）
pub fn foreground_process_name() -> Option<String> {
    None
}

/// 单次轮询（占位：前台未知 → 不动作）
pub fn tick(_app: &AppHandle) {}

/// 启动监控循环（占位：空转，不创建线程）
pub fn start_appwatch(_app: AppHandle) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_platform_agnostic() {
        assert_eq!(normalize_process_name("C:\\Apps\\PhotoShop.exe"), "photoshop");
        assert_eq!(normalize_process_name("/Applications/Safari.app"), "safari.app");
        assert_eq!(normalize_process_name("code"), "code");
    }

    #[test]
    fn watch_action_state_machine() {
        assert_eq!(watch_action(true, false), WatchAction::Suspend);
        assert_eq!(watch_action(true, true), WatchAction::Stay);
        assert_eq!(watch_action(false, true), WatchAction::Resume);
        assert_eq!(watch_action(false, false), WatchAction::None);
    }

    #[test]
    fn stub_foreground_is_none() {
        assert!(foreground_process_name().is_none());
        assert!(foreground_process_path().is_none());
    }
}
