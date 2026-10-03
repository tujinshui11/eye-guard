//! gdi32 gamma ramp 绑定——多显示器支持版
//!
//! 关键事实：Windows 的 gamma ramp 是 **per-monitor** 的；`GetDC(None)`（NULL）
//! 只关联主显示器——因此旧实现下**副屏完全不受滤镜影响**。本版改为：
//! - 写入：枚举所有已连接桌面的显示器（EnumDisplayDevicesW）→ 逐屏 CreateDC + SetDeviceGammaRamp
//! - 读取：仍读主屏 ramp 作为备份基准（各屏原始校准差异在恢复时以主屏值近似，
//!   见下方"已知限制"）
//!
//! 已知限制：异型/异校准多屏时，副屏的"原始 ramp"未被单独快照；恢复时会把
//! 主屏快照写回所有屏。多数同型号双屏场景无感知差异。
//!
//! 句柄纪律：CreateDC/DeleteDC 与 GetDC/ReleaseDC 严格配对，不缓存句柄。

use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, EnumDisplayDevicesW, GetDC, ReleaseDC, DISPLAY_DEVICEW, HDC,
};
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};

/// 枚举已连接桌面的显示器设备名（如 `\\.\DISPLAY1`）
pub fn display_device_names() -> Vec<String> {
    let mut names = Vec::new();
    unsafe {
        let mut i: u32 = 0;
        loop {
            let mut dd = DISPLAY_DEVICEW::default();
            dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            let ok = EnumDisplayDevicesW(PCWSTR::null(), i, &mut dd, 0);
            if !ok.as_bool() {
                break;
            }
            // DISPLAY_DEVICE_ATTACHED_TO_DESKTOP = 0x1
            if dd.StateFlags.0 & 0x1 != 0 {
                let end = dd
                    .DeviceName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(dd.DeviceName.len());
                let name = String::from_utf16_lossy(&dd.DeviceName[..end]);
                if !name.is_empty() {
                    names.push(name);
                }
            }
            i += 1;
            if i > 32 {
                break; // 防御：异常环境下避免无限循环
            }
        }
    }
    names
}

/// 打开指定显示器设备的 DC
fn dc_for_display(name: &str) -> Option<HDC> {
    let driver: Vec<u16> = "DISPLAY\0".encode_utf16().collect();
    let device: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hdc = CreateDCW(
            PCWSTR(driver.as_ptr()),
            PCWSTR(device.as_ptr()),
            PCWSTR::null(),
            None,
        );
        if hdc.is_invalid() {
            None
        } else {
            Some(hdc)
        }
    }
}

/// 读取当前 gamma ramp（主屏；768 项：R0..R255, G0..G255, B0..B255，每项 0-65535）
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

/// 写入 gamma ramp 到**所有**已连接显示器
/// @returns true = 至少一个显示器写入成功
pub fn write_ramp(lut: &[u16; 768]) -> bool {
    let names = display_device_names();
    let mut any_ok = false;
    for name in &names {
        if let Some(hdc) = dc_for_display(name) {
            unsafe {
                let ok = SetDeviceGammaRamp(hdc, lut.as_ptr() as *const c_void);
                let _ = DeleteDC(hdc);
                if ok.as_bool() {
                    any_ok = true;
                }
            }
        }
    }
    if any_ok {
        return true;
    }
    // 枚举失败或全部失败：退回主屏 DC 路径（至少保证主屏行为与旧版一致）
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 枚举至少应找到一个已连接桌面的显示器（开发机/CI 均有屏幕或虚拟屏）
    #[test]
    fn display_names_not_empty() {
        let names = display_device_names();
        assert!(!names.is_empty(), "应至少枚举到一个显示器设备");
        // 设备名形如 \\.\DISPLAY1
        assert!(names.iter().all(|n| n.starts_with("\\\\.\\DISPLAY")), "设备名格式异常: {names:?}");
    }

    /// 单次读-写-回写不 panic（真实环境冒烟；驱动不支持读取时允许 Err）
    #[test]
    fn read_write_smoke() {
        let r = read_ramp();
        if let Ok(buf) = r {
            let ok = write_ramp(&buf);
            assert!(ok, "回写自身 ramp 应成功");
        }
    }
}
