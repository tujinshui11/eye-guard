//! gamma（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：CoreGraphics `CGSetDisplayTransferByTable`
//! （注意：macOS 对 gamma 修改有 SIP 限制，同类工具多用私有 API，需真机验证）
//!
//! 当前为占位：读返回 Err、写返回 false —— 调用方（display.rs）已有降级路径，
//! 表现为"色温调节不可用"，不会崩溃。

/// 读取当前 gamma ramp（占位：不支持）
pub fn read_ramp() -> Result<[u16; 768], String> {
    Err("当前平台暂不支持 gamma 读取".to_string())
}

/// 写入 gamma ramp（占位：不支持）
pub fn write_ramp(_lut: &[u16; 768]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unsupported_without_panic() {
        assert!(read_ramp().is_err());
        assert!(!write_ramp(&[0u16; 768]));
    }
}
