//! gdi32 gamma ramp 绑定——对照移植 `src/main/gamma.js`（koffi FFI → windows crate 直绑）
//!
//! 偏离记录：JS 版在模块内缓存屏幕 DC 复用（避免重复获取）；Rust 版每次
//! GetDC/ReleaseDC 配对——无句柄泄漏、无静态可变状态，功能等价（GetDC 开销极低）。

use std::ffi::c_void;
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};

/// 读取当前 gamma ramp（768 项：R0..R255, G0..G255, B0..B255，每项 0-65535）
pub fn read_ramp() -> Result<[u16; 768], String> {
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Err("GetDC(null) 返回空句柄".to_string());
        }
        let mut buf = [0u16; 768];
        let ok = GetDeviceGammaRamp(hdc, buf.as_mut_ptr() as *mut c_void);
        let _ = ReleaseDC(None, hdc);
        if !ok.as_bool() {
            return Err("GetDeviceGammaRamp 调用失败（显卡驱动不支持 gamma 读取）".to_string());
        }
        Ok(buf)
    }
}

/// 写入 gamma ramp
/// @returns true = 驱动接受；false = 驱动拒绝（如超出支持范围）
pub fn write_ramp(lut: &[u16; 768]) -> bool {
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return false;
        }
        let ok = SetDeviceGammaRamp(hdc, lut.as_ptr() as *const c_void);
        let _ = ReleaseDC(None, hdc);
        ok.as_bool()
    }
}
