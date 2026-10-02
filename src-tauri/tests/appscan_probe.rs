//! 应用扫描探针（manual，--ignored）——打印本机扫描结果
//!
//! 运行：cargo test --test appscan_probe -- --ignored --nocapture

#[test]
#[ignore = "手动探针：打印本机已安装应用扫描结果"]
fn scan_installed_apps_probe() {
    let apps = eye_guard_lib::appscan::scan_installed_apps();
    println!("[probe] 共扫描到 {} 个应用：", apps.len());
    for a in apps.iter().take(40) {
        println!("  - {} ({})", a.name, a.process);
    }
    if apps.len() > 40 {
        println!("  ... 其余 {} 个省略", apps.len() - 40);
    }
    assert!(!apps.is_empty(), "扫描不应为空");
}
