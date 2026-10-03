//! appicon（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：读 `.app/Contents/Resources/*.icns` → 转 PNG data URL
//!
//! 当前为占位：返回 None（前端回落到"中性灰块"占位，已有该路径）。

/// 提取图标为 data URL（占位：不支持）
pub fn extract_icon_data_url(_exe_path: &str) -> Option<String> {
    None
}

/// 提取 RGBA 像素（占位：不支持）
pub fn extract_icon_rgba(_exe_path: &str) -> Option<Vec<u8>> {
    None
}

/// 诊断变体：不反预乘（占位：不支持）
pub fn extract_icon_rgba_nopremul(_exe_path: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_icon_extraction_returns_none() {
        assert!(extract_icon_data_url("/Applications/Safari.app").is_none());
        assert!(extract_icon_rgba("x").is_none());
    }
}
