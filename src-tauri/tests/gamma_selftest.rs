//! 真实屏幕 gamma 写读回自检——对照移植 scripts/gamma-selftest.js 的精神
//!
//! 手动运行（会短暂改变屏幕色温后复原）：
//!   cargo test --release --test gamma_selftest -- --ignored --nocapture
//!
//! 覆盖：常规暖区（4500K）+ 冷区（8000K / 10000K）写入 → 读回逐元素比对 → 复原。

use eye_guard_lib::gamma;
use eye_guard_lib::temperature::build_safe_lut;

#[test]
#[ignore = "真实屏幕自检：需手动运行（会短暂改变屏幕色温）"]
fn gamma_selftest_real_screen() {
    let original = gamma::read_ramp().expect("读取原始 ramp 失败");
    println!("[selftest] 原始 ramp 已存档");

    for k in [4500.0_f64, 8000.0, 10000.0] {
        let result = build_safe_lut(&original, k, 100.0);
        let ok = gamma::write_ramp(&result.lut);
        assert!(ok, "{k}K 写入被驱动拒绝");
        std::thread::sleep(std::time::Duration::from_millis(500));

        let readback = gamma::read_ramp().expect("读回失败");
        assert_eq!(
            readback, result.lut,
            "{k}K 读回与写入不一致（clamped={}, effective={}K）",
            result.clamped, result.effective_temperature
        );
        println!(
            "[selftest] {k}K → 写入/读回一致（clamped={}, effective={}K）",
            result.clamped, result.effective_temperature
        );
    }

    // 复原
    assert!(gamma::write_ramp(&original), "复原原始 ramp 失败");
    let restored = gamma::read_ramp().expect("复原读回失败");
    assert_eq!(restored, original, "复原后与原始 ramp 不一致");
    println!("[selftest] 已复原原始色彩 ✓");
}
