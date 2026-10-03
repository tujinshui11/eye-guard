//! 感光监测模块组（W3）：analyzer / als / camera（纯逻辑与采样后端）
//! monitor（采样循环壳）在 W3-c 接线批加入
//
// 平台门控：als 依赖 WinRT（Windows）、camera 依赖 nokhwa（Windows feature），
// 非 Windows 平台走 platform/stub 的降级实现（ALS 报告无硬件 → 上层回退摄像头；
// 摄像头在非 Windows 尚不可用 → 感光监测整体不可用，不影响其它功能）。

#[cfg(windows)]
pub mod als;
#[cfg(not(windows))]
#[path = "../platform/stub/als.rs"]
pub mod als;

#[cfg(windows)]
pub mod camera;
#[cfg(not(windows))]
#[path = "../platform/stub/camera.rs"]
pub mod camera;

pub mod analyzer;
pub mod monitor;
