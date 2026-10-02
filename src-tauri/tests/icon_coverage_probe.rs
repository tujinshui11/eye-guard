//! 图标覆盖率统计探针（manual，--ignored）
//!
//! 运行：cargo test --test icon_coverage_probe -- --ignored --nocapture

#[test]
#[ignore = "手动探针：统计扫描结果的图标覆盖率"]
fn icon_coverage() {
    let apps = eye_guard_lib::appscan::scan_installed_apps();
    let with_icon = apps.iter().filter(|a| a.icon.is_some()).count();
    let total = apps.len();
    println!("[coverage] 共 {total} 个应用，其中 {with_icon} 个带图标");
    println!(
        "[coverage] 覆盖率 {:.0}%",
        (with_icon as f64 / total.max(1) as f64) * 100.0
    );
    // 打印几个样例验证 URL 格式
    for a in apps.iter().take(5) {
        let icon_info = match &a.icon {
            Some(u) => {
                let prefix_ok = u.starts_with("data:image/png;base64,");
                format!("有图标 (前缀正确={prefix_ok}, 长度={})", u.len())
            }
            None => "无图标".to_string(),
        };
        println!("  - {} ({}): {icon_info}", a.name, a.process);
    }
    assert!(
        with_icon * 2 > total,
        "图标覆盖率应过半（实得 {with_icon}/{total}）"
    );
}
