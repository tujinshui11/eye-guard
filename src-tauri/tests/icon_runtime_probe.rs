//! 图标运行时环境复现探针（manual，--ignored）
//!
//! 目的：检验「Tauri 运行时线程 COM 状态影响图标提取」的假设。
//! 假设（已证伪）：async 命令线程被亮度命令初始化为 MTA 后，扫描链的
//! STA 初始化失败（RPC_E_CHANGED_MODE），导致 Shell 调用行为异常。
//!
//! 【结论 2026-10-02】MTA 污染线程与干净线程结果完全一致（75 项 / 66 图标），
//! 该假设不成立。真正根因是 index.html 的 CSP `default-src 'self'` 拦截了
//! `data:` 图标（已修复为加 `img-src 'self' data:`）。本探针保留作对照：
//! MTA 环境下提取行为应与干净线程一致（防止未来回归到这里）。
//!
//! 运行：cargo test --test icon_runtime_probe -- --ignored --nocapture

#[test]
#[ignore = "复现探针：MTA 污染线程下的扫描行为"]
fn scan_under_mta_polluted_thread() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    // 模拟：亮度命令先把本线程初始化为 MTA（Tauri 线程池复用的真实场景）
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    println!("[probe] 1st CoInitializeEx(MTA): {hr:?}");

    // 同一线程上执行扫描（内部 resolve_lnk_target 会尝试 STA 初始化）
    let apps = eye_guard_lib::appscan::scan_installed_apps();
    let with_icon = apps.iter().filter(|a| a.icon.is_some()).count();
    println!(
        "[probe] MTA 线程下：{} 项，{} 个有图标",
        apps.len(),
        with_icon
    );
    for a in apps.iter().take(5) {
        println!("  - {} ({}): icon={}", a.name, a.process, a.icon.is_some());
    }
    assert!(!apps.is_empty(), "列表不应为空（lnk 解析失败会让列表空）");
}

#[test]
#[ignore = "复现探针：干净线程下的扫描行为（对照组）"]
fn scan_under_clean_thread() {
    let apps = eye_guard_lib::appscan::scan_installed_apps();
    let with_icon = apps.iter().filter(|a| a.icon.is_some()).count();
    println!(
        "[probe] 干净线程下：{} 项，{} 个有图标",
        apps.len(),
        with_icon
    );
    assert!(!apps.is_empty());
}
