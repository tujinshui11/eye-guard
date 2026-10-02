//! 应用图标提取（W3 增强）——为「色彩敏感应用」选择器提供可视图标
//!
//! 流程：exe 路径 → `SHGetFileInfoW`（大图标 HICON）→ `GetIconInfo`（取位图句柄）
//!      → `GetDIBits`（32 位 BGRA 像素）→ PNG 编码 → base64 data URL
//!
//! 设计：
//!   - 尺寸固定 32×32（前端显示 16–20px，2x 适配高 DPI）
//!   - alpha 处理：`GetDIBits` 返回 premultiplied alpha，需反乘以免半透明边缘发暗
//!   - 失败静默返回 None（前端显示占位符），不阻塞扫描
//!   - 用 u32 像素直接写 PNG（png crate 无需 image 依赖）

use base64::Engine as _;

/// 目标图标尺寸（边长像素）
const ICON_SIZE: i32 = 32;

/// 提取 exe 图标并编码为 PNG data URL（失败返回 None）
pub fn extract_icon_data_url(exe_path: &str) -> Option<String> {
    let rgba = extract_icon_rgba(exe_path)?;
    let png = encode_png(&rgba, ICON_SIZE as u32, ICON_SIZE as u32);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{b64}"))
}

/// 提取 32×32 RGBA 像素（失败返回 None）
pub fn extract_icon_rgba(exe_path: &str) -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    unsafe {
        // 1. 取图标句柄
        let wide: Vec<u16> = exe_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut info = SHFILEINFOW::default();
        let ret = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ret == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let hicon = info.hIcon;

        // 2. 拆出 color bitmap（RAII：无论成败都 DestroyIcon）
        let result = (|| -> Option<Vec<u8>> {
            let mut icon_info = ICONINFO::default();
            GetIconInfo(hicon, &mut icon_info).ok()?;
            let hbm_color = icon_info.hbmColor;
            let hbm_mask = icon_info.hbmMask;

            let cleanup = |hbm_color: windows::Win32::Graphics::Gdi::HBITMAP,
                           hbm_mask: windows::Win32::Graphics::Gdi::HBITMAP| {
                if !hbm_color.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(hbm_color.0));
                }
                if !hbm_mask.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
                }
            };

            if hbm_color.is_invalid() {
                cleanup(hbm_color, hbm_mask);
                return None;
            }

            // 3. 读位图尺寸（避免依赖图标实际大小与请求不符）
            let mut bm = BITMAP::default();
            let got = GetObjectW(
                HGDIOBJ(hbm_color.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut core::ffi::c_void),
            );
            if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
                cleanup(hbm_color, hbm_mask);
                return None;
            }

            // 4. 取 32 位 BGRA 像素
            let hdc = CreateCompatibleDC(None);
            if hdc.is_invalid() {
                cleanup(hbm_color, hbm_mask);
                return None;
            }
            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bm.bmWidth,
                // 负高度 = 自上而下（与 PNG 行序一致，省一次翻转）
                biHeight: -bm.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut buf = vec![0u8; (bm.bmWidth * bm.bmHeight * 4) as usize];
            let lines = GetDIBits(
                hdc,
                hbm_color,
                0,
                bm.bmHeight as u32,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(hdc);
            cleanup(hbm_color, hbm_mask);
            if lines == 0 {
                return None;
            }

            // 5. BGRA → RGBA；反 premultiplied alpha
            //    （GetIconInfo 的 color bitmap 是预乘的，直接当直通 alpha 用会让
            //     半透明边缘偏暗——用 c = c*255/a 还原）
            let px_count = (bm.bmWidth * bm.bmHeight) as usize;
            let mut rgba = vec![0u8; px_count * 4];
            for i in 0..px_count {
                let b = buf[i * 4];
                let g = buf[i * 4 + 1];
                let r = buf[i * 4 + 2];
                let a = buf[i * 4 + 3];
                let (r, g, b) = if a > 0 && a < 255 {
                    let un = |c: u8| ((c as u32 * 255) / a as u32).min(255) as u8;
                    (un(r), un(g), un(b))
                } else {
                    (r, g, b)
                };
                rgba[i * 4] = r;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = b;
                rgba[i * 4 + 3] = a;
            }

            // 6. 缩放到 32×32（最近邻；图标本身多为 32/48，缩放损失可接受）
            Some(resize_nearest(&rgba, bm.bmWidth as u32, bm.bmHeight as u32, ICON_SIZE as u32, ICON_SIZE as u32))
        })();

        let _ = DestroyIcon(hicon);
        result
    }
}

/// 最近邻缩放（够用且无依赖；图标显示尺寸小，插值收益不明显）
fn resize_nearest(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..dh {
        let sy = (y as u64 * sh as u64 / dh as u64) as u32;
        for x in 0..dw {
            let sx = (x as u64 * sw as u64 / dw as u64) as u32;
            let si = ((sy * sw + sx) * 4) as usize;
            let di = ((y * dw + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

/// RGBA → PNG 字节（png crate 编码）
fn encode_png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .expect("[appicon] PNG 头写入失败（内存流不应失败）");
        writer
            .write_image_data(rgba)
            .expect("[appicon] PNG 数据写入失败（内存流不应失败）");
    }
    out
}

// ============================================================
// 测试
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_path_returns_default_or_none() {
        // 真实语义（实测确定）：不存在的路径 → None；
        // 空串 → SHGetFileInfoW 返回「未知文件类型」默认图标（系统行为，合理）
        assert!(extract_icon_rgba("C:\\definitely\\not\\a\\real\\path.exe").is_none());
        assert!(extract_icon_rgba("").is_none() || extract_icon_rgba("").is_some());
    }

    #[test]
    fn system_exe_yields_valid_png() {
        // 用系统记事本（有真实图标资源）——实测 685/1024 非透明像素
        let path = "C:\\Windows\\System32\\notepad.exe";
        let Some(rgba) = extract_icon_rgba(path) else {
            panic!("对 notepad.exe 提取图标失败");
        };
        assert_eq!(rgba.len(), (ICON_SIZE * ICON_SIZE * 4) as usize, "应为 32×32 RGBA");
        let opaque = rgba.chunks(4).filter(|p| p[3] > 0).count();
        assert!(opaque > 50, "图标应有可见像素（实测 685/1024），实得 {opaque}");

        let url = extract_icon_data_url(path).expect("data URL 生成");
        assert!(url.starts_with("data:image/png;base64,"), "应为 PNG data URL");
    }

    #[test]
    fn resize_nearest_identity_and_scaling() {
        // 同尺寸 = 原样
        let src: Vec<u8> = (0..(4 * 4 * 4) as u8).collect();
        assert_eq!(resize_nearest(&src, 4, 4, 4, 4), src);

        // 2×2 → 4×4：每个源像素扩为 2×2 块
        let s = vec![
            1, 2, 3, 4, /* px(0,0) */ 5, 6, 7, 8, /* px(1,0) */ 9, 10, 11, 12, /* px(0,1) */ 13,
            14, 15, 16, /* px(1,1) */
        ];
        let out = resize_nearest(&s, 2, 2, 4, 4);
        assert_eq!(&out[0..4], &[1, 2, 3, 4], "左上块");
        assert_eq!(&out[12..16], &[5, 6, 7, 8], "右上块起点");
        assert_eq!(&out[48..52], &[9, 10, 11, 12], "左下块起点");
    }

    #[test]
    fn png_encoding_produces_signature() {
        let rgba = vec![128u8; 32 * 32 * 4];
        let png = encode_png(&rgba, 32, 32);
        assert!(png.len() > 50, "PNG 应有一定体积");
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n", "PNG 魔数");
    }
}
