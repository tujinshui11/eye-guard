//! 摄像头测光——对照移植 `src/renderer/camera.js` 的采样语义（nokhwa 直采样）
//!
//! 实现变更：JS 版经隐藏窗口 + getUserMedia（~1.5s/次）；Rust 版 nokhwa 直抓帧，
//! 并丢弃预热帧（H1 探针实测：首帧曝光未收敛 luma=3.0）。
//!
//! 采样语义（与 camera.js 一致）：
//!   64×64 网格最近邻降采样 → 每像素 Rec.709 感知亮度 → 平均 → 两位小数
//!   fail-closed：任何失败返回 {ok:false, error}，从不 panic；采样结束立即释放设备

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

/// 单次采样结果
#[derive(Debug, Clone)]
pub struct SampleResult {
    pub ok: bool,
    pub luma: Option<f64>,
    pub error: Option<String>,
}

fn fail(msg: String) -> SampleResult {
    SampleResult {
        ok: false,
        luma: None,
        error: Some(msg),
    }
}

/// 单次测光采样（阻塞，约 0.5–1s；调用方控制周期）
pub fn sample_luma() -> SampleResult {
    let index = CameraIndex::Index(0);
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    let mut camera = match Camera::new(index, requested) {
        Ok(c) => c,
        Err(e) => return fail(format!("打开摄像头失败: {e:?}")),
    };

    // 预热：丢弃首帧（曝光未收敛——H1 探针实测首帧 luma=3.0）
    if let Err(e) = camera.frame() {
        return fail(format!("预热帧失败: {e:?}"));
    }
    let frame = match camera.frame() {
        Ok(f) => f,
        Err(e) => return fail(format!("抓帧失败: {e:?}")),
    };
    let decoded = match frame.decode_image::<RgbFormat>() {
        Ok(img) => img,
        Err(e) => return fail(format!("RGB 解码失败: {e:?}")),
    };

    // 64×64 网格最近邻采样 → 4096 像素 Rec.709 平均（对照 drawImage 缩放语义）
    let (w, h) = (decoded.width(), decoded.height());
    if w == 0 || h == 0 {
        return fail("帧尺寸为 0".to_string());
    }
    let mut sum = 0.0f64;
    for gy in 0..64u32 {
        let y = (gy * h / 64).min(h - 1);
        for gx in 0..64u32 {
            let x = (gx * w / 64).min(w - 1);
            let p = decoded.get_pixel(x, y);
            sum += 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        }
    }
    let luma = (sum / 4096.0 * 100.0).round() / 100.0;

    drop(camera); // 立即释放（fail 路径由 Drop 自动释放）
    SampleResult {
        ok: true,
        luma: Some(luma),
        error: None,
    }
}
