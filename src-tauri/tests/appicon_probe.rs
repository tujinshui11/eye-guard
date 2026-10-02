//! 图标提取诊断探针（manual，--ignored）——逐步骤定位失败点
//!
//! 运行：cargo test --test appicon_probe -- --ignored --nocapture

#[test]
#[ignore = "手动探针：诊断图标提取链路"]
fn icon_extraction_diagnosis() {
    // 1. 空路径行为
    println!("=== 空路径 ===");
    println!("  extract(empty) = {:?}", eye_guard_lib::appicon::extract_icon_rgba(""));

    // 2. 不存在的路径
    println!("=== 不存在路径 ===");
    let missing = eye_guard_lib::appicon::extract_icon_rgba("C:\\not\\real\\app.exe");
    println!("  extract(missing) = {:?}", missing.is_some());

    // 3. 真实系统 exe（有图标资源）
    for path in [
        "C:\\Windows\\System32\\notepad.exe",
        "C:\\Windows\\explorer.exe",
        "C:\\Windows\\System32\\mspaint.exe",
    ] {
        let r = eye_guard_lib::appicon::extract_icon_rgba(path);
        println!(
            "=== {path} ===\n  result = {}",
            match &r {
                Some(v) => format!("Some({} bytes)", v.len()),
                None => "None".to_string(),
            }
        );
        if let Some(v) = &r {
            let opaque = v.chunks(4).filter(|p| p[3] > 0).count();
            println!("  非透明像素 = {opaque}/1024");
        }
    }

    // 4. 当前测试二进制（Rust 编译产物，通常无图标资源）
    let exe = std::env::current_exe().unwrap();
    let r = eye_guard_lib::appicon::extract_icon_rgba(&exe.to_string_lossy());
    println!("=== 测试二进制 ===\n  {} → {}", exe.display(), r.is_some());
}
