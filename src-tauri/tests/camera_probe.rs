//! 摄像头采样集成探针（camera.rs 完整链路；原 H1 单帧探针已被其取代）
//!
//! 手动运行（会短暂占用摄像头约 1 秒）：
//!   cargo test --release --test camera_probe -- --ignored --nocapture
//!
//! 注意：与其它摄像头测试**不要并行运行**（设备独占）。

/// camera.rs 完整采样链路集成探针（预热弃帧 + 64×64 降采样 + luma + 释放）
#[test]
#[ignore = "集成探针：需手动运行（会短暂占用摄像头）"]
fn camera_sample_luma_integration() {
    let res = eye_guard_lib::ambient::camera::sample_luma();
    println!(
        "[probe] sample_luma ok={} luma={:?} error={:?}",
        res.ok, res.luma, res.error
    );
    assert!(res.ok, "采样应成功: {:?}", res.error);
    let luma = res.luma.unwrap();
    assert!((0.0..=255.0).contains(&luma), "luma 越界: {luma}");
    println!("[probe] camera.rs 集成 PASS ✓");
}
