//! H1 探针：nokhwa 摄像头单帧抓取实测（W3 首个动作——决定 camera 直采样 vs 回退方案）
//!
//! 手动运行（会短暂占用摄像头约 1 秒）：
//!   cargo test --release --test camera_probe -- --ignored --nocapture
//!
//! 验证项：① 打开耗时 ② 抓帧耗时 ③ luma 计算链路 ④ 释放后立即可重开（设备释放干净）

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

fn open_camera() -> Result<Camera, String> {
    let index = CameraIndex::Index(0);
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    Camera::new(index, requested).map_err(|e| format!("{e:?}"))
}

#[test]
#[ignore = "H1 探针：需手动运行（会短暂占用摄像头）"]
fn nokhwa_single_frame_probe() {
    let t0 = std::time::Instant::now();
    let mut camera = match open_camera() {
        Ok(c) => c,
        Err(e) => panic!("H1 FAIL: 无法打开摄像头: {e}"),
    };
    let open_ms = t0.elapsed().as_millis();

    let t1 = std::time::Instant::now();
    let frame = camera.frame().expect("抓帧失败");
    let frame_ms = t1.elapsed().as_millis();
    let resolution = frame.resolution();
    let decoded = frame.decode_image::<RgbFormat>().expect("RGB 解码失败");

    // 64×64 降采样 → 感知亮度 luma（Rec.709 权重）
    let (w, h) = (decoded.width(), decoded.height());
    let step_x = (w / 64).max(1) as usize;
    let step_y = (h / 64).max(1) as usize;
    let mut sum = 0.0f64;
    let mut count = 0u32;
    let mut y = 0u32;
    while y < h {
        let mut x = 0u32;
        while x < w {
            let p = decoded.get_pixel(x, y);
            sum += 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
            count += 1;
            x += step_x as u32;
        }
        y += step_y as u32;
    }
    let luma = sum / count as f64;

    drop(camera);

    // 释放干净验证：立即重开
    let t2 = std::time::Instant::now();
    let reopen = open_camera();
    let reopen_ms = t2.elapsed().as_millis();
    let reopen_ok = reopen.is_ok();
    drop(reopen);

    println!(
        "[probe] open={open_ms}ms frame={frame_ms}ms res={resolution:?} samples={count} luma={luma:.1} reopen_ok={reopen_ok} reopen={reopen_ms}ms"
    );

    assert!((0.0..=255.0).contains(&luma), "luma 越界: {luma}");
    assert!(reopen_ok, "设备释放后应立即可重开");
    println!("[probe] H1 PASS ✓");
}
