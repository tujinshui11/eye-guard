//! 日落跟随——太阳时刻计算（NOAA 简化算法）与过渡曲线
//!
//! 纯函数模块：
//! - `sun_times`：给定纬度/经度/时区/日期 → 当地时间日出日落（分钟数；极昼极夜返回 None）
//! - `transition_progress`：余弦缓入缓出（ease-in-out）——模拟真实天光变化率（慢→快→慢）
//! - `lerp`：色温/亮度插值
//!
//! 精度目标：与权威数据（Open-Meteo / NOAA）误差 < 2 分钟——测试以实抓 API 数据为黄金用例。

use std::f64::consts::PI;

/// 日出/日落结果：自当地 0 点起的分钟数（None = 极昼或极夜）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunTimes {
    pub sunrise_min: Option<f64>,
    pub sunset_min: Option<f64>,
}

/// 判定"日落"的太阳高度角修正：-0.833°（大气折射 34′ + 日盘半径 16′）
const SUNSET_ZENITH_DEG: f64 = 90.833;

/// 地球轨道偏心率
const ECCENTRICITY: f64 = 0.016_708_634;

/// 某日某地的日出日落（NOAA 简化算法）
///
/// `tz_offset_hours`：当地时区偏移（北京 +8，UTC+8）。
pub fn sun_times(
    lat: f64,
    lon: f64,
    tz_offset_hours: f64,
    year: i32,
    month: u32,
    day: u32,
) -> SunTimes {
    // 儒略日（以当地正午为参考瞬间）
    let jd = julian_day(year, month, day) - tz_offset_hours / 24.0;

    // 自 J2000 的世纪数
    let t = (jd - 2_451_545.0) / 36_525.0;

    // 太阳几何平黄经 / 平近点角（度）
    let l0 = 280.466_46 + t * (36_000.769_83 + t * 0.000_303_2);
    let m = 357.529_11 + t * (35_999.050_29 - 0.000_153_7 * t);

    // 中心差 → 真黄经
    let m_rad = m.to_radians();
    let c = m_rad.sin() * (1.914_602 - t * (0.004_817 + 0.000_014 * t))
        + (2.0 * m_rad).sin() * (0.019_993 - 0.000_101 * t)
        + (3.0 * m_rad).sin() * 0.000_289;
    let true_lon = l0 + c;

    // 黄经章动修正 → 视黄经
    let omega = 125.04 - 1_934.136 * t;
    let app_lon = true_lon - 0.005_69 - 0.004_78 * omega.to_radians().sin();

    // 黄赤交角（度）→ 太阳赤纬（度）
    let eps0 = 23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.000_59 - t * 0.001_813))) / 60.0) / 60.0;
    let eps = eps0 + 0.002_56 * omega.to_radians().cos();
    let decl = (eps.to_radians().sin() * app_lon.to_radians().sin())
        .asin()
        .to_degrees();

    // 时差方程（度）
    let y = (eps / 2.0).to_radians().tan().powi(2);
    let l0_rad = l0.to_radians();
    let eot_deg = y * (2.0 * l0_rad).sin() - 2.0 * ECCENTRICITY * m_rad.sin()
        + 4.0 * ECCENTRICITY * y * m_rad.sin() * (2.0 * l0_rad).cos()
        - 0.5 * y * y * (4.0 * l0_rad).sin()
        - 1.25 * ECCENTRICITY * ECCENTRICITY * (2.0 * m_rad).sin();
    let eot_min = 4.0 * eot_deg.to_degrees();

    // 时角（度）
    let lat_rad = lat.to_radians();
    let decl_rad = decl.to_radians();
    let cos_ha = SUNSET_ZENITH_DEG.to_radians().cos() / (lat_rad.cos() * decl_rad.cos())
        - lat_rad.tan() * decl_rad.tan();

    // 极昼（cos_ha < -1）/ 极夜（cos_ha > 1）
    if !(-1.0..=1.0).contains(&cos_ha) {
        return SunTimes {
            sunrise_min: None,
            sunset_min: None,
        };
    }

    let ha_deg = cos_ha.acos().to_degrees();
    // 当地标准时正午（分钟）：720 - 4°·经度 + 60·时区小时 - 时差
    // （等价 NOAA 的 720 + 4·timezoneDeg - 4·longitude - eqtime，timezoneDeg = 15·tz_hours）
    let solar_noon = 720.0 - 4.0 * lon + 60.0 * tz_offset_hours - eot_min;
    SunTimes {
        sunrise_min: Some(solar_noon - 4.0 * ha_deg),
        sunset_min: Some(solar_noon + 4.0 * ha_deg),
    }
}

/// 儒略日（UTC 0 时；格里高利历）
fn julian_day(year: i32, month: u32, day: u32) -> f64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = (y as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y as f64 + 4_716.0)).floor() + (30.600_1 * (m as f64 + 1.0)).floor() + day as f64 + b
        - 1_524.5
}

/// 过渡进度（余弦 ease-in-out）：elapsed ∈ [0, window] → p ∈ [0, 1]
///
/// 起手轻、中段匀、收尾柔——与真实天光变化率（慢→快→慢）一致。
pub fn transition_progress(elapsed_ms: i64, window_ms: i64) -> f64 {
    if window_ms <= 0 || elapsed_ms <= 0 {
        return 0.0;
    }
    if elapsed_ms >= window_ms {
        return 1.0;
    }
    let t = elapsed_ms as f64 / window_ms as f64;
    (1.0 - (PI * t).cos()) / 2.0
}

/// 线性插值（色温/亮度共用）
pub fn lerp(from: f64, to: f64, p: f64) -> f64 {
    from + (to - from) * p
}

// ============================================================
// 测试
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 黄金用例：Open-Meteo 实抓数据（北京 39.9N/116.4E，UTC+8，2026-10-03..09 日落时刻）
    /// 参考（API）：17:54 / 17:52 / 17:51 / 17:49 / 17:47 / 17:46 / 17:44
    const BEIJING_SUNSET: [(u32, f64); 7] = [
        (3, 17.0 * 60.0 + 54.0),
        (4, 17.0 * 60.0 + 52.0),
        (5, 17.0 * 60.0 + 51.0),
        (6, 17.0 * 60.0 + 49.0),
        (7, 17.0 * 60.0 + 47.0),
        (8, 17.0 * 60.0 + 46.0),
        (9, 17.0 * 60.0 + 44.0),
    ];

    #[test]
    fn beijing_sunset_matches_reference_within_3min() {
        // 容差 3 分钟：Open-Meteo 参考值为整分钟舍入（±0.5 分），
        // NOAA 简化算法本身精度约 1–2 分钟——3 分钟为工程合理线。
        for (day, expected_min) in BEIJING_SUNSET {
            let st = sun_times(39.9, 116.4, 8.0, 2026, 10, day);
            let sunset = st.sunset_min.expect("北京十月应有日落");
            assert!(
                (sunset - expected_min).abs() < 3.0,
                "10-{day}: 计算 {sunset:.1} 分 vs 参考 {expected_min:.1} 分（差 {:.1}）",
                (sunset - expected_min).abs()
            );
        }
    }

    /// 北京同一日的日出也应在合理范围（API: 06:12 → 372 分）
    #[test]
    fn beijing_sunrise_matches_reference_within_2min() {
        let st = sun_times(39.9, 116.4, 8.0, 2026, 10, 3);
        let sunrise = st.sunrise_min.expect("北京十月应有日出");
        assert!(
            (sunrise - 372.0).abs() < 2.0,
            "计算 {sunrise:.1} 分 vs 参考 372.0 分"
        );
    }

    /// 南半球（悉尼 2026-10 春）：日落应在 17:00–20:00 的合理区间
    /// （实测 10 月初约 17:58，注意南半球春季日落尚未过 18 点）
    #[test]
    fn sydney_sunset_plausible() {
        let st = sun_times(-33.87, 151.21, 10.0, 2026, 10, 3);
        let sunset = st.sunset_min.expect("悉尼应有日落");
        assert!(
            sunset > 17.0 * 60.0 && sunset < 20.0 * 60.0,
            "悉尼日落 {sunset:.1} 分不合理"
        );
    }

    /// 高纬度极夜：朗伊尔城（78.2N）12 月至次年 2 月无日出日落
    #[test]
    fn high_latitude_polar_night_returns_none() {
        let st = sun_times(78.22, 15.65, 1.0, 2026, 12, 21);
        assert!(st.sunset_min.is_none() && st.sunrise_min.is_none(), "极夜应返回 None");
    }

    /// 高纬度极昼：朗伊尔城 6 月无日落
    #[test]
    fn high_latitude_polar_day_returns_none() {
        let st = sun_times(78.22, 15.65, 2.0, 2026, 6, 21);
        assert!(st.sunset_min.is_none(), "极昼应返回 None");
    }

    // ---- 过渡曲线 ----

    #[test]
    fn progress_endpoints_exact() {
        assert_eq!(transition_progress(0, 3_600_000), 0.0);
        assert_eq!(transition_progress(-5, 3_600_000), 0.0);
        assert_eq!(transition_progress(3_600_000, 3_600_000), 1.0);
        assert_eq!(transition_progress(9_999_999, 3_600_000), 1.0);
    }

    #[test]
    fn progress_midpoint_is_half() {
        let p = transition_progress(1_800_000, 3_600_000);
        assert!((p - 0.5).abs() < 1e-9, "中点应精确 0.5，实际 {p}");
    }

    #[test]
    fn progress_monotonic_nondecreasing() {
        let mut prev = -1.0;
        for i in 0..=120 {
            let p = transition_progress(i * 30_000, 3_600_000);
            assert!(p >= prev, "曲线应单调不减（i={i}）");
            prev = p;
        }
    }

    #[test]
    fn progress_zero_window_is_safe() {
        assert_eq!(transition_progress(1000, 0), 0.0);
        assert_eq!(transition_progress(1000, -10), 0.0);
    }

    /// 「丝滑」的数学判据：60 分钟窗口、30s 采样下，单步色温变化远超低于人眼可觉差。
    /// 取色温跨度 3000K（6500 → 3500 的极端情形）验证最坏步长：
    /// 余弦曲线峰值步长约 39K ≈ 1.1%@3500K，远低于色温 JND（约 3–5%，>100K）。
    #[test]
    fn max_step_below_perceptual_threshold() {
        let window_ms = 60 * 60 * 1000;
        let step_ms = 30 * 1000;
        let span = 3000.0_f64;
        let mut prev = lerp(6500.0, 3500.0, transition_progress(0, window_ms));
        let mut max_delta = 0.0_f64;
        let mut t = step_ms;
        while t <= window_ms {
            let cur = lerp(6500.0, 3500.0, transition_progress(t, window_ms));
            max_delta = max_delta.max((cur - prev).abs());
            prev = cur;
            t += step_ms;
        }
        assert!(
            max_delta < 60.0,
            "最大步长 {max_delta:.2}K 应低于 60K（≈1.8%@3400K，JND 安全线）"
        );
        assert!(span > 0.0);
    }

    #[test]
    fn lerp_basic() {
        assert_eq!(lerp(10.0, 20.0, 0.0), 10.0);
        assert_eq!(lerp(10.0, 20.0, 1.0), 20.0);
        assert_eq!(lerp(10.0, 20.0, 0.5), 15.0);
    }
}
