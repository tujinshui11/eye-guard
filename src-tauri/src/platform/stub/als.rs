//! ALS（环境光传感器）——非 Windows 平台降级实现
//!
//! macOS 真实现方向：Apple 未公开通用 ALS API（部分机型可经 IOKit 私有接口读取）；
//! 实务做法是**直接依赖摄像头测光**（ambient/camera.rs），或提示用户不可用。
//!
//! 占位语义：`available=false` → 上层（monitor.rs）会自然回退到摄像头路径。

/// ALS 读取结果（三态，与 Windows 版字段一致）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsReading {
    pub available: bool,
    pub lux: Option<f64>,
}

/// 读取一次 ALS（占位：无硬件）
pub fn read_als() -> AlsReading {
    AlsReading {
        available: false,
        lux: None,
    }
}

/// 探测是否有 ALS 硬件（占位：false → 回退摄像头）
pub fn detect_als() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_no_hardware() {
        let r = read_als();
        assert!(!r.available);
        assert!(r.lux.is_none());
        assert!(!detect_als(), "占位应报告无 ALS，让上层回退摄像头");
    }
}
