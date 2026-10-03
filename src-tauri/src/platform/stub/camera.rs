//! 摄像头测光——非 Windows 平台降级实现
//!
//! 真实现方向：macOS 可用 AVFoundation 抓帧（或经现有 nokhwa 的 macOS backend）；
//! 本占位返回明确错误，使上层（monitor.rs）如实报告"感光监测不可用"。

/// 单次采样结果（字段与 Windows 版一致）
#[derive(Debug, Clone)]
pub struct SampleResult {
    pub ok: bool,
    pub luma: Option<f64>,
    pub error: Option<String>,
}

/// 单次测光采样（占位：明确报告不支持，不静默失败）
pub fn sample_luma() -> SampleResult {
    SampleResult {
        ok: false,
        luma: None,
        error: Some("当前平台暂不支持摄像头测光".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unsupported_explicitly() {
        let r = sample_luma();
        assert!(!r.ok);
        assert!(r.luma.is_none());
        assert!(r.error.is_some(), "应给出明确原因而非静默失败");
    }
}
