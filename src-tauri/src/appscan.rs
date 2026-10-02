//! 已安装应用扫描（W3 增强）——供「色彩敏感应用」选择器使用
//!
//! 数据源：开始菜单两处快捷方式目录（用户级 + 系统级）的 `.lnk` 文件，
//! 用 COM `IShellLinkW` + `IPersistFile` 解析出目标 exe → 归一化进程名。
//!
//! 设计：
//!   - 只读扫描，失败项静默跳过（个别损坏快捷方式不影响整体）
//!   - 去重（同名 exe 只保留一条）、过滤系统噪音（uninstall/help 等）
//!   - 结果按名称排序，供前端搜索框过滤

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 一条可选项：显示名 + 进程名（白名单存储用进程名）+ 图标
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppEntry {
    /// 显示名（快捷方式文件名去扩展名，如 "Adobe Photoshop 2024"）
    pub name: String,
    /// 归一化进程名（如 "photoshop"，写入白名单用这个）
    pub process: String,
    /// 图标 data URL（PNG base64；提取失败为 None）
    pub icon: Option<String>,
}

/// 过滤系统噪音：这些不是用户想加白名单的"色彩敏感应用"
fn is_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    const NOISE: [&str; 9] = [
        "uninstall",
        "卸载",
        "readme",
        "help",
        "帮助",
        "website",
        "官网",
        "documentation",
        "release notes",
    ];
    NOISE.iter().any(|n| lower.contains(n))
}

/// 扫描开始菜单目录下的所有 .lnk（递归一层即可覆盖常见布局）
fn collect_lnk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // 递归子目录（开始菜单通常两层：厂商/应用）
            collect_lnk_files(&path, out);
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// 解析单个 .lnk 的目标路径（COM IShellLinkW + IPersistFile）
pub fn resolve_lnk_target(lnk: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Option<String> {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let persist: IPersistFile = link.cast().ok()?;

            let wide: Vec<u16> = lnk
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

            let mut buf = [0u16; 512];
            link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if len == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..len]))
        })();

        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

/// 扫描已安装应用（供前端选择器渲染）
///
/// 返回按显示名排序的去重列表。当前会话内无缓存——调用频率低（仅面板打开时），
/// 全量扫描耗时约数百毫秒可接受。
pub fn scan_installed_apps() -> Vec<AppEntry> {
    let mut lnks = Vec::new();
    let dirs: Vec<PathBuf> = [
        std::env::var_os("APPDATA").map(|p| {
            PathBuf::from(p)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
        }),
        std::env::var_os("ProgramData").map(|p| {
            PathBuf::from(p)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
        }),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in &dirs {
        collect_lnk_files(dir, &mut lnks);
    }

    // BTreeMap 以进程名为键去重（同进程多个快捷方式只留一条），值取 (显示名, 目标exe)
    let mut by_process: BTreeMap<String, (String, String)> = BTreeMap::new();
    for lnk in lnks {
        let Some(display) = lnk.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if is_noise(display) {
            continue;
        }
        let Some(target) = resolve_lnk_target(&lnk) else {
            continue;
        };
        // .msc 系统管理单元（services.msc 等）——对"色彩敏感应用"白名单无意义
        if target.to_ascii_lowercase().ends_with(".msc") {
            continue;
        }
        let process = crate::appwatch::normalize_process_name(&target);
        // 过滤明显非应用的（空名 / 卸载器 / rundll32 等）
        if process.is_empty()
            || process == "uninstall"
            || process == "rundll32"
            || process == "cmd"
            || process == "powershell"
        {
            continue;
        }
        by_process
            .entry(process)
            .or_insert_with(|| (display.to_string(), target));
    }

    // 排序：按显示名（不区分大小写）；图标提取（每条失败静默 None）
    let mut list: Vec<AppEntry> = by_process
        .into_iter()
        .map(|(process, (name, target))| AppEntry {
            name,
            process,
            icon: crate::appicon::extract_icon_data_url(&target),
        })
        .collect();
    list.sort_by_key(|e| e.name.to_ascii_lowercase());
    list
}

// ============================================================
// 测试
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_filter_blocks_system_entries() {
        assert!(is_noise("Uninstall MyApp"));
        assert!(is_noise("卸载助手"));
        assert!(is_noise("README"));
        assert!(is_noise("Help Center"));
        assert!(!is_noise("Adobe Photoshop 2024"));
        assert!(!is_noise("剪映专业版"));
    }

    #[test]
    fn scan_returns_sorted_deduped_list() {
        // 与 appicon 的 Shell 测试串行（见 lib.rs test_support：Shell 并发竞态）
        let _g = crate::test_support::SHELL_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        // 实机冒烟：真实扫描本机开始菜单
        let apps = scan_installed_apps();
        // 有开始菜单就一定有个位数以上条目
        assert!(!apps.is_empty(), "扫描结果不应为空（本机有开始菜单）");
        // 排序检查（不区分大小写升序）
        for w in apps.windows(2) {
            assert!(
                w[0].name.to_ascii_lowercase() <= w[1].name.to_ascii_lowercase(),
                "应按名称排序: {} > {}",
                w[0].name,
                w[1].name
            );
        }
        // 去重检查（进程名唯一）
        let mut procs: Vec<&str> = apps.iter().map(|a| a.process.as_str()).collect();
        let before = procs.len();
        procs.sort_unstable();
        procs.dedup();
        assert_eq!(before, procs.len(), "进程名应唯一（已去重）");
    }

    #[test]
    fn scan_filters_noise_and_normalizes() {
        // 与 appicon 的 Shell 测试串行（见 lib.rs test_support：Shell 并发竞态）
        let _g = crate::test_support::SHELL_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let apps = scan_installed_apps();
        for a in &apps {
            assert!(!a.process.is_empty());
            assert!(!a.process.ends_with(".exe"), "进程名应已去后缀: {}", a.process);
            assert!(!a.process.contains('\\'), "进程名不应含路径: {}", a.process);
            assert!(!is_noise(&a.name), "不应含噪音项: {}", a.name);
        }
    }

    // ============================================================
    // 诊断探针（--ignored）：图标质量检查
    // ============================================================
    mod probe {
        use super::*;
        use base64::Engine as _;

        fn stats(url: &str) -> String {
            let Some(b64) = url.strip_prefix("data:image/png;base64,") else {
                return "not-a-png-dataurl".into();
            };
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) else {
                return "b64-decode-fail".into();
            };
            let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
            let Ok(mut reader) = decoder.read_info() else {
                return "png-header-fail".into();
            };
            let mut buf = vec![0; reader.output_buffer_size()];
            let Ok(info) = reader.next_frame(&mut buf) else {
                return "png-frame-fail".into();
            };
            let px = &buf[..info.buffer_size()];
            let n = px.len() / 4;
            let a_pos = px.chunks(4).filter(|c| c[3] > 0).count();
            let a_255 = px.chunks(4).filter(|c| c[3] == 255).count();
            let colorful = px
                .chunks(4)
                .filter(|c| c[3] > 0)
                .filter(|c| {
                    let mx = c[0].max(c[1]).max(c[2]) as i32;
                    let mn = c[0].min(c[1]).min(c[2]) as i32;
                    mx - mn > 40
                })
                .count();
            format!("{n}px alpha>0:{a_pos} opaque:{a_255} colorful:{colorful}")
        }

        fn direct(p: &str) -> String {
            match crate::appicon::extract_icon_data_url(p) {
                Some(u) => stats(&u),
                None => "NONE".into(),
            }
        }

        #[test]
        #[ignore = "诊断探针：检查真实应用图标的 alpha/彩色像素分布"]
        fn probe_icon_quality() {
            println!("=== A. 直接路径提取 ===");
            for p in [
                "C:\\Windows\\System32\\cleanmgr.exe",
                "C:\\Windows\\System32\\dfrgui.exe",
                "C:\\Windows\\System32\\eventvwr.msc",
                "C:\\Windows\\System32\\compmgmt.msc",
                "C:\\Windows\\explorer.exe",
                "C:\\Windows\\System32\\notepad.exe",
            ] {
                let r: String = direct(p);
                println!("[direct] {p}: {r}");
            }

            println!("=== B. 扫描结果（UI 实际拿到的）===");
            let apps = scan_installed_apps();
            println!("[scan] total={}", apps.len());
            for name in [
                "clash-verge",
                "chrome",
                "cleanmgr",
                "dfrgui",
                "7zfm",
                "explorer",
            ] {
                let r = match apps.iter().find(|a| a.process == name) {
                    Some(a) => match &a.icon {
                        Some(u) => stats(u),
                        None => "icon=NONE".into(),
                    },
                    None => "not-found-in-scan".into(),
                };
                println!("[scan] {name}: {r}");
            }

            // === C. 导出 PNG 到磁盘供人工查看（决定性证据）===
            println!("=== C. 导出 PNG ===");
            let out_dir = std::path::Path::new("C:\\Users\\Admin\\AppData\\Local\\Temp");
            for (name, proc) in [
                ("clash-verge", "clash-verge"),
                ("cleanmgr", "cleanmgr"),
                ("eventvwr", "eventvwr"),
                ("chrome", "chrome"),
            ] {
                let Some(app) = apps.iter().find(|a| a.process == proc) else {
                    println!("[dump] {proc}: not found");
                    continue;
                };
                let Some(url) = &app.icon else {
                    println!("[dump] {proc}: icon=NONE");
                    continue;
                };
                let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
                let path = out_dir.join(format!("eyeguard-icon-{name}.png"));
                std::fs::write(&path, &bytes).unwrap();
                println!("[dump] {} → {} ({} bytes)", proc, path.display(), bytes.len());
            }
        }
    }
}
