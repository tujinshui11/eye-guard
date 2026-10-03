//! 日落跟随运行时——过渡窗口调度（复用 30s tick，不新起线程）
//!
//! 行为设计（「丝滑」三要素）：
//! 1. 余弦缓入缓出曲线（见 `sun::transition_progress`）——慢→快→慢，与真实天光一致
//! 2. 亮度错峰：亮度用 0.8× 居中窗口（`brightness_progress`）——两维不同时变
//! 3. 唤醒补过渡：睡眠跨窗后由 tick 的 jumped 标志触发 3 分钟快速收敛
//!
//! 尊重手动：用户手动改档/色温/亮度后 1 小时内不覆盖（`last_manual_at` 打点）。

use crate::sun::{lerp, sun_times, transition_progress};
use chrono::{Datelike, Timelike};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// 过渡方向：Dusk=日落变暖，Dawn=日出回升
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionDir {
    Dusk,
    Dawn,
}

/// 过渡状态（纯函数输出）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionState {
    pub dir: TransitionDir,
    /// 色温进度 0..1
    pub progress: f64,
}

/// 判定当前是否处于过渡窗口（纯函数；now/sunrise/sunset 均为当地分钟数 0..1440）
pub fn resolve_transition(
    now_min: f64,
    sunrise_min: f64,
    sunset_min: f64,
    window_min: f64,
) -> Option<TransitionState> {
    if window_min <= 0.0 || sunrise_min >= sunset_min {
        return None;
    }
    let w_ms = (window_min * 60_000.0) as i64;

    // 傍晚窗口：[sunset - W, sunset]
    if now_min >= sunset_min - window_min && now_min <= sunset_min {
        let elapsed = ((now_min - (sunset_min - window_min)) * 60_000.0) as i64;
        return Some(TransitionState {
            dir: TransitionDir::Dusk,
            progress: transition_progress(elapsed, w_ms),
        });
    }

    // 早晨窗口：[sunrise, sunrise + W]
    if now_min >= sunrise_min && now_min <= sunrise_min + window_min {
        let elapsed = ((now_min - sunrise_min) * 60_000.0) as i64;
        return Some(TransitionState {
            dir: TransitionDir::Dawn,
            progress: transition_progress(elapsed, w_ms),
        });
    }

    None
}

/// 亮度错峰窗口比例（0.8×，居中）——色温先动、亮度中段跟进，避免两维同时变的合成突变感
const BRIGHTNESS_WINDOW_RATIO: f64 = 0.8;

/// 把色温进度映射为亮度进度（更窄窗口 + 再次 ease）
pub fn brightness_progress(p: f64) -> f64 {
    let margin = (1.0 - BRIGHTNESS_WINDOW_RATIO) / 2.0;
    let t = ((p - margin) / BRIGHTNESS_WINDOW_RATIO).clamp(0.0, 1.0);
    transition_progress((t * 1_000.0) as i64, 1_000)
}

/// 读取日落跟随配置（未启用返回 None）
fn read_config(app: &AppHandle) -> Option<(f64, f64, String, f64)> {
    let state = app.state::<Mutex<crate::state::AppState>>();
    let st = state.lock().unwrap();
    let s = st.settings.get();
    let sun = s.get("sunFollow")?;
    if !sun.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    let lat = sun.get("lat").and_then(|v| v.as_f64())?;
    let lon = sun.get("lon").and_then(|v| v.as_f64())?;
    let target = sun
        .get("targetModeId")
        .and_then(|v| v.as_str())
        .unwrap_or("evening")
        .to_string();
    // 过渡窗口固定 60 分钟（与前端一致；忽略历史配置值，避免前后端口径分裂）
    const WINDOW_MINUTES: f64 = 60.0;
    Some((lat, lon, target, WINDOW_MINUTES))
}

/// 手动操作静默期（毫秒）：用户手动调整后此时长内不覆盖
const MANUAL_GRACE_MS: i64 = 60 * 60 * 1000;

/// 单次检查（由 schedule tick 调用）
pub fn check_sun_follow(app: &AppHandle, now_ms: i64, jumped: bool) {
    let Some((lat, lon, target_id, window_min)) = read_config(app) else {
        return;
    };
    // 手动优先：静默期内跳过
    {
        let state = app.state::<Mutex<crate::state::AppState>>();
        let st = state.lock().unwrap();
        if let Some(t) = st.last_manual_at {
            if now_ms - t < MANUAL_GRACE_MS {
                return;
            }
        }
    }

    let now = chrono::Local::now();
    let tz_hours = now.offset().local_minus_utc() as f64 / 3600.0;
    let times = sun_times(lat, lon, tz_hours, now.year(), now.month(), now.day());
    let (Some(sunrise), Some(sunset)) = (times.sunrise_min, times.sunset_min) else {
        return; // 极昼/极夜：跳过
    };

    let now_min = now.hour() as f64 * 60.0 + now.minute() as f64 + now.second() as f64 / 60.0;
    let Some(st) = resolve_transition(now_min, sunrise, sunset, window_min) else {
        // 睡眠唤醒且已越过日落/日出（错过整个窗口）：直接收敛到应处值。
        // 唤醒瞬间用户刚睁眼、对屏幕状态无预期，此处瞬间应用不构成突兀——
        // 比"3 分钟补过渡动画"简单且诚实（旧实现的分支恒为 no-op，已移除）。
        if jumped {
            if let Some(mode) = crate::modes::get_mode(&target_id) {
                let night_side = now_min > sunset || now_min < sunrise;
                let (k, b) = if night_side {
                    (mode.kelvin as f64, mode.brightness)
                } else {
                    (6500.0, 100)
                };
                let cur = {
                    let state = app.state::<Mutex<crate::state::AppState>>();
                    let stt = state.lock().unwrap();
                    stt.display.current_temperature
                };
                if (cur - k).abs() >= 1.0 {
                    apply_sun_ephemeral(app, k, b);
                    eprintln!("[sun] 唤醒收敛 → {k:.0}K（错过过渡窗口）");
                }
            }
        }
        return;
    };

    // 目标与起点：Dusk → 目标档；Dawn → 回升到原色 6500K
    let Some(mode) = crate::modes::get_mode(&target_id) else {
        return;
    };
    let (target_k, target_b) = (mode.kelvin as f64, mode.brightness as f64);
    let (from_k, from_b) = match st.dir {
        TransitionDir::Dusk => (6500.0_f64, 100.0_f64),
        TransitionDir::Dawn => (target_k, target_b),
    };
    let (to_k, to_b) = match st.dir {
        TransitionDir::Dusk => (target_k, target_b),
        TransitionDir::Dawn => (6500.0_f64, 100.0_f64),
    };

    let progress = st.progress;
    let kelvin = lerp(from_k, to_k, progress);
    let brightness = lerp(from_b, to_b, brightness_progress(progress)).round() as u32;

    // 幂等：色温与亮度都与当前值一致时跳过（避免无谓的 gamma 重写）
    {
        let state = app.state::<Mutex<crate::state::AppState>>();
        let stt = state.lock().unwrap();
        if (stt.display.current_temperature - kelvin).abs() < 1.0
            && stt.overlay_brightness == brightness
        {
            return;
        }
    }

    apply_sun_ephemeral(app, kelvin, brightness);
    eprintln!(
        "[sun] {:?} p={:.3} → {:.0}K / {}%（日落 {:.0} 分 / 现在 {:.0} 分）",
        st.dir, progress, kelvin, brightness, sunset, now_min
    );
}

/// 日落跟随的临时应用路径：只改显示层（gamma + 背光 + 黑纱），**不写 settings**
/// （避免把伪模式名写进 settings.modeId——审查 HIGH-3；同时让亮度错峰真正落地——HIGH-1）
fn apply_sun_ephemeral(app: &AppHandle, kelvin: f64, brightness: u32) {
    let (alpha, payload) = {
        let state = app.state::<Mutex<crate::state::AppState>>();
        let mut st = state.lock().unwrap();
        let _ = st.display.apply(kelvin);
        let (_outcome, alpha) = st.brightness.apply(brightness);
        st.overlay_brightness = brightness;
        (alpha, crate::state::display_payload(&st))
    };
    crate::overlay::apply_mask_alpha(app, alpha);
    let _ = app.emit("display:changed", payload);
}

// ============================================================
// 测试——窗口判定与错峰映射（纯函数）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    const SUNRISE: f64 = 6.0 * 60.0; // 06:00
    const SUNSET: f64 = 18.0 * 60.0; // 18:00

    #[test]
    fn outside_window_returns_none() {
        assert!(resolve_transition(12.0 * 60.0, SUNRISE, SUNSET, 60.0).is_none());
        // 16:30 在窗口 [17:00, 18:00] 之外
        assert!(resolve_transition(16.5 * 60.0, SUNRISE, SUNSET, 60.0).is_none());
        assert!(resolve_transition(19.0 * 60.0, SUNRISE, SUNSET, 60.0).is_none());
    }

    #[test]
    fn dusk_window_endpoints() {
        let start = resolve_transition(17.0 * 60.0, SUNRISE, SUNSET, 60.0).expect("窗口起点");
        assert_eq!(start.dir, TransitionDir::Dusk);
        assert_eq!(start.progress, 0.0);

        let end = resolve_transition(18.0 * 60.0, SUNRISE, SUNSET, 60.0).expect("窗口终点");
        assert_eq!(end.progress, 1.0);
    }

    #[test]
    fn dusk_midpoint_is_half() {
        let mid = resolve_transition(17.5 * 60.0, SUNRISE, SUNSET, 60.0).expect("窗口中点");
        assert!((mid.progress - 0.5).abs() < 1e-9, "实际 {}", mid.progress);
    }

    #[test]
    fn dawn_window_direction_and_progress() {
        let start = resolve_transition(6.0 * 60.0, SUNRISE, SUNSET, 60.0).expect("日出起点");
        assert_eq!(start.dir, TransitionDir::Dawn);
        assert_eq!(start.progress, 0.0);

        let end = resolve_transition(7.0 * 60.0, SUNRISE, SUNSET, 60.0).expect("日出终点");
        assert_eq!(end.progress, 1.0);
    }

    #[test]
    fn zero_window_or_degenerate_returns_none() {
        assert!(resolve_transition(18.0 * 60.0, SUNRISE, SUNSET, 0.0).is_none());
        // sunrise >= sunset（数据异常）→ None
        assert!(resolve_transition(12.0 * 60.0, SUNSET, SUNRISE, 60.0).is_none());
    }

    #[test]
    fn brightness_progress_endpoints_and_monotonic() {
        assert_eq!(brightness_progress(0.0), 0.0);
        assert_eq!(brightness_progress(1.0), 1.0);
        let mut prev = -1.0;
        for i in 0..=100 {
            let p = brightness_progress(i as f64 / 100.0);
            assert!(p >= prev, "亮度映射应单调不减（i={i}）");
            prev = p;
        }
    }

    #[test]
    fn brightness_lags_temperature_midway() {
        // 色温进度 0.5 时，亮度进度应 < 0.5（错峰：亮度更晚跟进）
        let b = brightness_progress(0.5);
        assert!(b < 0.5, "亮度进度 {b} 应落后于色温 0.5");
    }
}
