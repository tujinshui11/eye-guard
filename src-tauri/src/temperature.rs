//! 色温算法——对照移植 `src/main/temperature.js`（黑体辐射近似，Tanner Helland 常数，6500K 归一）
//!
//! 纯函数模块：无 I/O、无副作用，可独立单测。
//! 驱动安全钳制（50% 规则）：每通道最大值必须 ≥ SAFE_MAX_VALUE（32768），
//! build_safe_lut 对被压过低的通道自动提升总增益，并回报实际生效色温。

/// 本机实测的驱动接受边界：每通道最大值必须 ≥ 该值（否则 SetDeviceGammaRamp 整体拒绝）
pub const SAFE_MAX_VALUE: u16 = 32768;

/// RGB 三元组（f64 尺度，对应 JS 的 { r, g, b }）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// build_safe_lut 的返回（对应 JS 的 { lut, clamped, effectiveTemperature, effectiveGains }）
#[derive(Debug, Clone)]
pub struct SafeLutResult {
    pub lut: [u16; 768],
    pub clamped: bool,
    pub effective_temperature: f64,
    pub effective_gains: Rgb,
}

/// 黑体辐射近似：色温 K → RGB 分量（0-255 尺度，已 clamp）
pub fn rgb_at_kelvin(k: f64) -> Rgb {
    let t = k / 100.0;
    let (r, g);
    if t <= 66.0 {
        r = 255.0;
        g = 99.4708025861 * t.ln() - 161.1195681661;
    } else {
        r = 329.698727446 * (t - 60.0).powf(-0.1332047592);
        g = 288.1221695283 * (t - 60.0).powf(-0.0755148492);
    }
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.5177312231 * (t - 10.0).ln() - 305.0447927307
    };
    Rgb {
        r: r.clamp(0.0, 255.0),
        g: g.clamp(0.0, 255.0),
        b: b.clamp(0.0, 255.0),
    }
}

/// 色温增益：relative to 6500K，恒在 [0,1]（只衰减，不增亮）；gain(6500) === (1,1,1) 精确成立
pub fn temperature_gain(k: f64) -> Rgb {
    let cur = rgb_at_kelvin(k);
    let ref_6500 = rgb_at_kelvin(6500.0);
    Rgb {
        r: (cur.r / ref_6500.r).clamp(0.0, 1.0),
        g: (cur.g / ref_6500.g).clamp(0.0, 1.0),
        b: (cur.b / ref_6500.b).clamp(0.0, 1.0),
    }
}

/// 生成 3×256 gamma LUT（[R0..R255, G0..G255, B0..B255]）
pub fn build_lut(orig: &[u16; 768], temperature: f64, brightness: f64) -> [u16; 768] {
    let gain = temperature_gain(temperature);
    let brightness_factor = brightness / 100.0;
    let gains = [
        gain.r * brightness_factor,
        gain.g * brightness_factor,
        gain.b * brightness_factor,
    ];
    let mut out = [0u16; 768];
    for c in 0..3 {
        let g = gains[c];
        let base = c * 256;
        for i in 0..256 {
            out[base + i] = ((orig[base + i] as f64) * g).round().clamp(0.0, 65535.0) as u16;
        }
    }
    out
}

/// 通道最大值（原始 ramp 通常单调，但按实际扫描——对照 channelMax）
fn channel_max(orig: &[u16; 768], channel: usize) -> u16 {
    let base = channel * 256;
    let mut m = 0u16;
    for i in 0..256 {
        let v = orig[base + i];
        if v > m {
            m = v;
        }
    }
    m
}

/// 反解：给定蓝通道增益，求等效色温 K（暖区；100K 取整）
pub fn kelvin_for_gain_b(target_gain: f64) -> f64 {
    if target_gain >= 1.0 {
        return 6500.0;
    }
    if target_gain <= 0.0 {
        return 2000.0;
    }
    let mut lo = 2000.0;
    let mut hi = 6500.0;
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if temperature_gain(mid).b < target_gain {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (((lo + hi) / 2.0) / 100.0).round() * 100.0
}

/// 冷区上界：10000K 处的红增益（≈0.791），作为 kelvin_for_gain_r 的判据下界
fn cold_r_gain_10000() -> f64 {
    temperature_gain(10000.0).r
}

/// 反解：给定红通道增益，求等效色温 K（冷区，100K 取整）
pub fn kelvin_for_gain_r(target_gain: f64) -> f64 {
    if target_gain >= 1.0 {
        return 6500.0;
    }
    if target_gain <= cold_r_gain_10000() {
        return 10000.0;
    }
    let mut lo = 6500.0;
    let mut hi = 10000.0;
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if temperature_gain(mid).r > target_gain {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (((lo + hi) / 2.0) / 100.0).round() * 100.0
}

/// 统一反解：由实际生效增益与亮度因子求等效色温 K（蓝通道优先）
pub fn equivalent_kelvin(eff_gain: Rgb, brightness_factor: f64) -> f64 {
    let bf = if brightness_factor == 0.0 {
        1.0
    } else {
        brightness_factor
    };
    let kb = (eff_gain.b / bf).min(1.0);
    let kr = (eff_gain.r / bf).min(1.0);
    if kb < 1.0 {
        return kelvin_for_gain_b(kb);
    }
    if kr < 1.0 {
        return kelvin_for_gain_r(kr);
    }
    6500.0
}

/// 安全 LUT：保证每通道最大值 ≥ SAFE_MAX_VALUE，并回报实际生效色温
pub fn build_safe_lut(orig: &[u16; 768], temperature: f64, brightness: f64) -> SafeLutResult {
    let gain = temperature_gain(temperature);
    let brightness_factor = brightness / 100.0;
    let mut eff = Rgb {
        r: gain.r * brightness_factor,
        g: gain.g * brightness_factor,
        b: gain.b * brightness_factor,
    };
    let mut clamped = false;

    let mut max_orig = [0f64; 3];
    for (c, slot) in max_orig.iter_mut().enumerate() {
        *slot = channel_max(orig, c) as f64;
    }
    for c in 0..3 {
        if max_orig[c] == 0.0 {
            continue;
        }
        let min_total_gain = SAFE_MAX_VALUE as f64 / max_orig[c];
        let cur = match c {
            0 => eff.r,
            1 => eff.g,
            _ => eff.b,
        };
        if cur < min_total_gain {
            let raised = min_total_gain.min(1.0);
            match c {
                0 => eff.r = raised,
                1 => eff.g = raised,
                _ => eff.b = raised,
            }
            clamped = true;
        }
    }

    let mut lut = [0u16; 768];
    let gains = [eff.r, eff.g, eff.b];
    for c in 0..3 {
        let g = gains[c];
        let base = c * 256;
        for i in 0..256 {
            lut[base + i] = ((orig[base + i] as f64) * g).round().clamp(0.0, 65535.0) as u16;
        }
    }

    let effective_temperature = equivalent_kelvin(eff, brightness_factor);
    SafeLutResult {
        lut,
        clamped,
        effective_temperature,
        effective_gains: eff,
    }
}

// ============================================================
// 测试——对照移植 tests/temperature.test.js（22 用例）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn approx(actual: f64, expected: f64, tol: f64, msg: &str) {
        assert!(
            (actual - expected).abs() <= tol,
            "{} expected {} ±{}, got {}",
            msg,
            expected,
            tol,
            actual
        );
    }

    fn std_ramp() -> [u16; 768] {
        let mut r = [0u16; 768];
        for (i, v) in r.iter_mut().enumerate() {
            *v = ((i % 256) * 257) as u16;
        }
        r
    }

    // ---- 基础 ----

    // 'rgbAtKelvin: 6500K 参考白点（独立推导：g≈254.11, b≈250.04）'
    #[test]
    fn rgb_at_6500_reference_white() {
        let rgb = rgb_at_kelvin(6500.0);
        approx(rgb.r, 255.0, 0.001, "r");
        approx(rgb.g, 254.11, 0.05, "g");
        approx(rgb.b, 250.04, 0.05, "b");
    }

    // 'temperatureGain: 6500K 精确等于 (1,1,1)'
    #[test]
    fn gain_6500_exact_identity() {
        let gain = temperature_gain(6500.0);
        assert_eq!(gain.r, 1.0);
        assert_eq!(gain.g, 1.0);
        assert_eq!(gain.b, 1.0);
    }

    // 'temperatureGain: 2700K 暖色（独立推导：g≈0.656, b≈0.350）'
    #[test]
    fn gain_2700_warm() {
        let gain = temperature_gain(2700.0);
        approx(gain.r, 1.0, 0.001, "r");
        approx(gain.g, 0.656, 0.005, "g");
        approx(gain.b, 0.350, 0.005, "b");
    }

    // 'temperatureGain: 4500K 中等暖（独立推导：g≈0.856, b≈0.750）'
    #[test]
    fn gain_4500_medium_warm() {
        let gain = temperature_gain(4500.0);
        approx(gain.r, 1.0, 0.001, "r");
        approx(gain.g, 0.856, 0.005, "g");
        approx(gain.b, 0.750, 0.005, "b");
    }

    // 'temperatureGain: 增益恒在 [0,1]——只衰减不增亮'
    #[test]
    fn gain_within_unit_range() {
        let mut k = 2000.0;
        while k <= 6500.0 {
            let gain = temperature_gain(k);
            for (name, v) in [("r", gain.r), ("g", gain.g), ("b", gain.b)] {
                assert!((0.0..=1.0).contains(&v), "K={} {}={}", k, name, v);
            }
            k += 100.0;
        }
    }

    // 'temperatureGain: 蓝增益随 K 降低单调不增'
    #[test]
    fn blue_gain_monotonic_towards_warm() {
        let mut prev = f64::INFINITY;
        let mut k = 6500.0;
        while k >= 2000.0 {
            let b = temperature_gain(k).b;
            assert!(b <= prev + 1e-12, "K={} b={} prev={}", k, b, prev);
            prev = b;
            k -= 100.0;
        }
    }

    // ---- buildLut ----

    // 'buildLut: 6500K + 100% 亮度 => 与原始 ramp 逐元素一致（恒等）'
    #[test]
    fn build_lut_identity_at_6500() {
        let orig = std_ramp();
        let lut = build_lut(&orig, 6500.0, 100.0);
        assert_eq!(lut, orig);
    }

    // 'buildLut: 输出恒被 clamp 在 [0,65535]'
    #[test]
    fn build_lut_clamped_to_range() {
        let hi = build_lut(&[65535u16; 768], 2000.0, 50.0);
        for (i, v) in hi.iter().enumerate() {
            // u16 天然 ≤65535，此处断言与 JS 的 clamp 意图一致（≥0）
            let _ = (i, v);
        }
        let lo = build_lut(&[0u16; 768], 6500.0, 50.0);
        assert!(lo.iter().all(|&v| v == 0));
    }

    // 'buildLut: 6500K + 50% 亮度 => 各通道≈原始值一半'
    #[test]
    fn build_lut_half_brightness() {
        let orig = [65535u16; 768];
        let lut = build_lut(&orig, 6500.0, 50.0);
        approx(lut[255] as f64, 32768.0, 1.0, "R[255]");
        approx(lut[512 + 255] as f64, 32768.0, 1.0, "B[255]");
    }

    // 'buildLut: 2700K 时蓝通道显著低于红通道（暖色方向）'
    #[test]
    fn build_lut_warm_direction() {
        let orig = [65535u16; 768];
        let lut = build_lut(&orig, 2700.0, 100.0);
        let r = lut[255] as f64;
        let b = lut[512 + 255] as f64;
        assert!(b < r * 0.4, "B={} 应显著低于 R={}", b, r);
    }

    // ---- 驱动安全钳制 ----

    // 'buildSafeLut: 6500K+100%（原色）无需钳制，与原始一致'
    #[test]
    fn safe_lut_identity_no_clamp() {
        let orig = std_ramp();
        let res = build_safe_lut(&orig, 6500.0, 100.0);
        assert_eq!(res.lut, orig);
        assert!(!res.clamped);
    }

    // 'buildSafeLut: 4500K+100%（安全组合）不触发钳制'
    #[test]
    fn safe_lut_4500_no_clamp() {
        let orig = std_ramp();
        let res = build_safe_lut(&orig, 4500.0, 100.0);
        assert!(!res.clamped);
    }

    // 'buildSafeLut: 2700K+100%（越界组合）自动钳制——蓝通道最大值 ≥ 32768，生效色温被抬高'
    #[test]
    fn safe_lut_2700_clamped() {
        let orig = std_ramp();
        let res = build_safe_lut(&orig, 2700.0, 100.0);
        assert!(res.clamped);
        let b_max = *res.lut[512..].iter().max().unwrap();
        assert!(b_max >= 32768, "B max={} 应 ≥ 32768", b_max);
        assert!(
            res.effective_temperature > 2700.0,
            "生效色温 {} 应高于请求 2700",
            res.effective_temperature
        );
        assert!(
            res.effective_temperature <= 3400.0,
            "生效色温 {} 应在 3400 以内（实测边界≈3230）",
            res.effective_temperature
        );
    }

    // 'buildSafeLut: 2700K+50%（双重压低）同样被钳制且不越界'
    #[test]
    fn safe_lut_2700_50_clamped_all_channels() {
        let orig = std_ramp();
        let res = build_safe_lut(&orig, 2700.0, 50.0);
        assert!(res.clamped);
        for c in 0..3 {
            let c_max = *res.lut[c * 256..c * 256 + 256].iter().max().unwrap();
            assert!(c_max >= 32768, "通道{} max={} 应 ≥ 32768", c, c_max);
        }
    }

    // 'buildSafeLut: 安全组合下 4500K 的读值仍≈期望（钳制不误伤正常范围）'
    #[test]
    fn safe_lut_4500_values_expected() {
        let orig = [65535u16; 768];
        let res = build_safe_lut(&orig, 4500.0, 100.0);
        approx(res.lut[255] as f64, 65535.0, 1.0, "R[255]");
        approx(
            res.lut[512 + 255] as f64,
            (65535.0 * temperature_gain(4500.0).b).round(),
            1.0,
            "B[255]",
        );
    }

    // ---- 冷区扩展（2000–10000K）----

    // 'temperatureGain: 8000K 冷色（独立推导：r≈0.867, g≈0.904, b=1.0）'
    #[test]
    fn gain_8000_cool() {
        let gain = temperature_gain(8000.0);
        approx(gain.r, 0.867, 0.01, "r");
        approx(gain.g, 0.904, 0.01, "g");
        approx(gain.b, 1.0, 0.01, "b");
    }

    // 'temperatureGain: 10000K 冷色（独立推导：r≈0.791, g≈0.858, b=1.0）'
    #[test]
    fn gain_10000_cool() {
        let gain = temperature_gain(10000.0);
        approx(gain.r, 0.791, 0.01, "r");
        approx(gain.g, 0.858, 0.01, "g");
        approx(gain.b, 1.0, 0.01, "b");
    }

    // 'temperatureGain: 冷区红增益随 K 升高单调不增（8000→10000 逐档）'
    #[test]
    fn red_gain_monotonic_towards_cool() {
        let mut prev = f64::INFINITY;
        let mut k = 8000.0;
        while k <= 10000.0 {
            let r = temperature_gain(k).r;
            assert!(r <= prev + 1e-12, "K={} r={} prev={}", k, r, prev);
            prev = r;
            k += 100.0;
        }
    }

    // 'equivalentKelvin: 2000..10000 逐档往返（亮度 100，|等效-请求|≤100）'
    #[test]
    fn equivalent_kelvin_roundtrip() {
        let mut k = 2000.0;
        while k <= 10000.0 {
            let gain = temperature_gain(k);
            let eff = equivalent_kelvin(gain, 1.0);
            approx(eff, k, 100.0, &format!("K={}", k));
            k += 100.0;
        }
    }

    // 'equivalentKelvin: 亮度 60/80 抽测往返（|等效-请求|≤100）'
    #[test]
    fn equivalent_kelvin_roundtrip_dimmed() {
        for bf in [0.6f64, 0.8] {
            for k in [2000.0, 3000.0, 4500.0, 6500.0, 8000.0, 10000.0] {
                let gain = temperature_gain(k);
                let eff = equivalent_kelvin(
                    Rgb {
                        r: gain.r * bf,
                        g: gain.g * bf,
                        b: gain.b * bf,
                    },
                    bf,
                );
                approx(eff, k, 100.0, &format!("bf={} K={}", bf, k));
            }
        }
    }

    // 'kelvinForGainR: 边界（1→6500；极小值→10000）'
    #[test]
    fn kelvin_for_gain_r_bounds() {
        assert_eq!(kelvin_for_gain_r(1.0), 6500.0);
        assert_eq!(kelvin_for_gain_r(0.0), 10000.0);
        assert_eq!(kelvin_for_gain_r(0.5), 10000.0);
    }

    // 'kelvinForGainR: 10000K 处红增益≈0.791 反解回 10000'
    #[test]
    fn kelvin_for_gain_r_roundtrip_10000() {
        let r = temperature_gain(10000.0).r;
        approx(r, 0.791, 0.01, "r@10000");
        assert_eq!(kelvin_for_gain_r(r), 10000.0);
    }
}
