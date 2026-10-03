//! appscan（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：扫 `/Applications` 与 `~/Applications` 下的 `.app` bundle，
//! 读 `Contents/Info.plist`（CFBundleName / CFBundleExecutable）+ `Contents/Resources/*.icns`
//!
//! 当前为占位：返回空列表（前端"色彩敏感应用"列表为空，不影响其它功能）。

use serde::Serialize;

/// 一条可选项（字段与 Windows 版一致）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppEntry {
    pub name: String,
    pub process: String,
    pub icon: Option<String>,
}

/// 解析快捷方式目标（占位：不支持）
pub fn resolve_lnk_target(_lnk: &std::path::Path) -> Option<String> {
    None
}

/// 扫描已安装应用（占位：空列表）
pub fn scan_installed_apps() -> Vec<AppEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_scan_returns_empty_without_panic() {
        assert!(scan_installed_apps().is_empty());
        assert!(resolve_lnk_target(std::path::Path::new("/tmp/x.lnk")).is_none());
    }
}
