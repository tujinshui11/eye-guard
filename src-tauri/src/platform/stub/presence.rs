//! presence（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：
//! - 空闲：`CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType)`
//! - 全屏：`CGWindowListCopyWindowInfo` 判断前台窗口是否覆盖整屏
//!
//! 当前为占位：空闲恒为 false（不触发"人不在"顺延，行为保守）、全屏恒为 false（不压制提醒）。

use std::time::Duration;

/// 空闲时长（占位：None = 无法判定）
pub fn idle_duration() -> Option<Duration> {
    None
}

/// 是否空闲（占位：false —— 保守策略，宁可多提醒也不静默）
pub fn is_idle() -> bool {
    false
}

/// 是否处于"系统免打扰"（占位：未知按 false）
pub fn is_shell_do_not_disturb() -> bool {
    false
}

/// 刷新并返回全屏状态（占位：false）
pub fn refresh_fullscreen_state() -> bool {
    false
}

/// 缓存的全屏状态（占位：false）
pub fn fullscreen_cached() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_presence_is_conservative() {
        assert!(!is_idle(), "占位应保守：不判定为空闲");
        assert!(idle_duration().is_none());
        assert!(!refresh_fullscreen_state());
        assert!(!fullscreen_cached());
    }
}
