//! appwatch 前台进程探测探针（manual，--ignored）——读取当前前台窗口进程
//!
//! 运行：cargo test --test appwatch_probe -- --ignored --nocapture
//!
//! 无副作用（只读）；与应用内检测走完全相同的代码路径。

use eye_guard_lib::appwatch::{foreground_process_name, foreground_process_path, is_whitelisted};

#[test]
#[ignore = "硬件探针：需手动运行（读取当前前台窗口进程，无副作用）"]
fn foreground_probe() {
    let path = foreground_process_path();
    let name = foreground_process_name();
    println!("[probe] foreground path: {path:?}");
    println!("[probe] foreground name: {name:?}");
    let Some(name) = name else {
        panic!("前台进程探测返回 None——检查 GetForegroundWindow 链（当前是否无聚焦窗口？）");
    };
    assert!(!name.is_empty(), "归一化后的进程名不应为空");
    // 用真实前台进程名验证匹配函数端到端自洽
    let wl = vec![name.clone()];
    assert!(
        is_whitelisted(&name, &wl),
        "自身归一化名应匹配自身白名单"
    );
    println!("[probe] PASS：探测到前台进程「{name}」，匹配函数自洽");
}
