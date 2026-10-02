//! 图标像素 ASCII 可视化探针（manual，--ignored）
//!
//! 运行：cargo test --test icon_ascii_probe -- --ignored --nocapture
//!
//! 把提取的 32×32 图标逐像素渲染为 ASCII（亮度分档），肉眼检查：
//!   - 图标是否真实（有形状/图案）
//!   - 反预乘处理前后是否有可见差异（边缘是否被「洗白」）

fn render_ascii(rgba: &[u8], w: usize) -> String {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let mut out = String::new();
    for y in 0..(rgba.len() / 4 / w) {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = rgba[i + 3];
            if a < 16 {
                out.push(' ');
            } else {
                // 亮度（考虑 alpha 混合到白底/黑底都不影响相对形状判断，用均值）
                let lum = (rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32) / 3;
                let idx = (lum as usize * (RAMP.len() - 1)) / 255;
                out.push(RAMP[idx] as char);
            }
        }
        out.push('\n');
    }
    out
}

fn color_stats(rgba: &[u8]) -> String {
    let mut opaque = 0;
    let mut colorful = 0;
    for c in rgba.chunks(4) {
        if c[3] > 0 {
            opaque += 1;
            let mx = c[0].max(c[1]).max(c[2]) as i32;
            let mn = c[0].min(c[1]).min(c[2]) as i32;
            if mx - mn > 40 {
                colorful += 1;
            }
        }
    }
    format!("opaque={opaque} colorful={colorful}")
}

#[test]
#[ignore = "手动探针：ASCII 渲染图标像素"]
fn ascii_render_chrome() {
    let path = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
    let Some(rgba) = eye_guard_lib::appicon::extract_icon_rgba(path) else {
        // 回退：chrome 装在别处时用 clash-verge 或本地已知 exe
        println!("chrome.exe 不存在于默认路径，用 notepad 代替");
        let rgba = eye_guard_lib::appicon::extract_icon_rgba("C:\\Windows\\System32\\notepad.exe")
            .expect("notepad 提取");
        println!("{}", render_ascii(&rgba, 32));
        println!("stats: {}", color_stats(&rgba));
        return;
    };
    println!("=== Chrome（反预乘后）===");
    println!("{}", render_ascii(&rgba, 32));
    println!("stats: {}", color_stats(&rgba));

    let nop = eye_guard_lib::appicon::extract_icon_rgba_nopremul(path).unwrap();
    println!("=== Chrome（未反预乘）===");
    println!("{}", render_ascii(&nop, 32));
    println!("stats: {}", color_stats(&nop));

    // 语义化外观检查：Chrome 图标应包含红/绿/黄/蓝彩色像素
    let has_red = rgba.chunks(4).any(|c| c[3] > 0 && c[0] > 180 && c[1] < 120 && c[2] < 120);
    let has_green = rgba.chunks(4).any(|c| c[3] > 0 && c[1] > 150 && c[0] < 150 && c[2] < 150);
    let has_blue = rgba.chunks(4).any(|c| c[3] > 0 && c[2] > 180 && c[0] < 120);
    println!("Chrome 配色检查: red={has_red} green={has_green} blue={has_blue}");
    assert!(has_red && has_green && has_blue, "Chrome 图标应含红/绿/蓝三色");
}
