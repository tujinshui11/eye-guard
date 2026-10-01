//! 只读探针：验证"运行中的 Rust 版"已把 night(3400K) LUT 真实写入屏幕
//!
//! 手动运行（只读，不改屏幕）：
//!   cargo test --release --test active_ramp_probe -- --ignored --nocapture
//!
//! 原理：读当前屏幕 ramp，与 build_safe_lut(original, 3400K, 100) 精确比对。
//! 期望差异为 0（同一函数同输入，位级一致）；大差异则说明 ramp 被其他进程覆盖。

use eye_guard_lib::temperature::build_safe_lut;

#[test]
#[ignore = "只读探针：需手动运行（读取当前屏幕 ramp 与备份比对）"]
fn active_ramp_matches_night_mode() {
    let current = eye_guard_lib::gamma::read_ramp().expect("读取当前 ramp 失败");
    let dir = eye_guard_lib::state::data_dir();
    let backup: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("gamma-backup.json")).unwrap())
            .expect("读取 backup 失败");
    let ramp: Vec<u16> = backup["ramp"]
        .as_array()
        .expect("backup.ramp 缺失")
        .iter()
        .map(|v| v.as_u64().unwrap() as u16)
        .collect();
    let mut original = [0u16; 768];
    original.copy_from_slice(&ramp);

    let expected = build_safe_lut(&original, 3400.0, 100.0).lut;
    let diff: u64 = current
        .iter()
        .zip(expected.iter())
        .map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs())
        .sum();
    println!("[probe] active ramp vs expected(3400K) diff_sum = {diff}");

    assert_eq!(diff, 0, "ramp 与 3400K 期望不一致（diff_sum={diff}）——屏幕可能被其他程序覆盖");
    println!("[probe] 屏幕当前 = night(3400K) 精确匹配 ✓（屏幕确实已暖色）");
}
