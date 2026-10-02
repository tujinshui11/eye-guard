//! 用户在场性检测（W2）——空闲检测 + 全屏/演示免打扰
//!
//! 两个用途：
//!   1. **空闲监测**（Stretchly/ProjectEye 借鉴）：键鼠长时间无输入 → 用户离开，
//!      休息计时不计入（人不在不算用眼）。
//!   2. **全屏免打扰**（ProjectEye 借鉴）：全屏游戏/视频/演示时，
//!      不弹休息卡片（不打断沉浸场景）。
//!
//! 实现：Win32 `GetLastInputInfo`（空闲毫秒）+ `SHQueryUserNotificationState`
//! （系统级「是否处于全屏/演示/忙碌」状态，Shell 官方判定，比自己枚举窗口可靠）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 空闲阈值：超过此值视为「用户离开」
pub const IDLE_THRESHOLD_SECS: u64 = 120;

/// 全屏状态缓存（由 tick 更新；供 break_rt 决策读取）
static FULLSCREEN_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 读取系统空闲时长（键鼠最后一次输入至今）
///
/// 返回 None 表示 API 不可用（极罕见；调用方按「不空闲」处理）
pub fn idle_duration() -> Option<Duration> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        let now = GetTickCount();
        // dwTime 为 u32 毫秒时间戳；wrapping_sub 处理 49.7 天回卷
        let elapsed_ms = now.wrapping_sub(info.dwTime);
        Some(Duration::from_millis(elapsed_ms as u64))
    }
}

/// 是否处于空闲（用户离开）状态
pub fn is_idle() -> bool {
    idle_duration()
        .map(|d| d.as_secs() >= IDLE_THRESHOLD_SECS)
        .unwrap_or(false)
}

/// Shell 层免打扰判定：全屏应用 / 演示模式 / 忙碌（勿扰）
///
/// 覆盖 `QUNS_RUNNING_D3D_FULL_SCREEN`（全屏游戏/视频）、
/// `QUNS_PRESENTATION_MODE`（演示）、`QUNS_BUSY`（专注/勿扰）
pub fn is_shell_do_not_disturb() -> bool {
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };

    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => {
            state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_PRESENTATION_MODE
                || state == QUNS_BUSY
        }
        Err(_) => false,
    }
}

/// 更新全屏状态缓存（由休息 tick 周期调用）
pub fn refresh_fullscreen_state() -> bool {
    let active = is_shell_do_not_disturb();
    FULLSCREEN_ACTIVE.store(active, Ordering::SeqCst);
    active
}

/// 读全屏状态缓存（无系统调用；供高频决策路径使用）
pub fn fullscreen_cached() -> bool {
    FULLSCREEN_ACTIVE.load(Ordering::SeqCst)
}

// ============================================================
// 测试：纯逻辑部分（系统 API 路径需实机；缓存语义可测）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_cache_roundtrip() {
        // 直接操纵缓存验证读写（不调系统 API，保持测试无副作用）
        FULLSCREEN_ACTIVE.store(true, Ordering::SeqCst);
        assert!(fullscreen_cached());
        FULLSCREEN_ACTIVE.store(false, Ordering::SeqCst);
        assert!(!fullscreen_cached());
    }

    #[test]
    fn idle_duration_callable() {
        // 实机冒烟：API 可调用且返回合理值（< 1 天）
        if let Some(d) = idle_duration() {
            assert!(d.as_secs() < 86_400, "空闲时长异常：{d:?}");
        }
    }
}
