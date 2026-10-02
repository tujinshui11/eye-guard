//! 亮度硬件探针（manual，--ignored）——真实 WMI 读写往返验证
//!
//! 运行：cargo test --test brightness_probe -- --ignored --nocapture
//!
//! 行为：诊断读路径 → 读原值 → 写 70 → 读回校验 → 恢复原值 → 读回校验。
//! 期间屏幕亮度会真实变化（约 1.5 秒），完成后恢复。

use eye_guard_lib::brightness::{BacklightIo, WmiBacklight};

/// 诊断读数路径（先跑这个定位问题）
#[test]
#[ignore = "硬件探针：需手动运行（只读诊断，不改屏幕状态）"]
fn brightness_diag() {
    let out = eye_guard_lib::brightness::debug_read_brightness();
    println!("[diag] 读取路径诊断：\n{out}");
}

/// 诊断写路径：写回当前值（不实际改亮度，安全）
#[test]
#[ignore = "硬件探针：需手动运行（写回当前值，屏幕无可见变化）"]
fn brightness_write_diag() {
    let current = match eye_guard_lib::brightness::debug_read_brightness() {
        _ => WmiBacklight.get(),
    };
    let v = current.unwrap_or(100);
    println!("[diag] 当前背光 {v}，写回同值诊断：");
    let (ok, log) = eye_guard_lib::brightness::set_brightness_diag(v as u8);
    println!("[diag] 结果 ok={ok}\n{log}");
    assert!(ok, "写路径失败，见上方诊断");
}

/// 完整往返（读→写→读→恢复→读）
#[test]
#[ignore = "硬件探针：需手动运行（真实改变屏幕亮度约 1.5 秒）"]
fn brightness_probe() {
    let io = WmiBacklight;

    let original = io.get();
    println!("[probe] 原始背光: {original:?}");
    let Some(original) = original else {
        println!("[probe] 本机无 WMI 背光（外接屏/台式机路径）——该路径由纯黑纱兜底");
        panic!("读取路径返回 None——请先跑 brightness_diag 定位（本机 PowerShelli 可读到背光）");
    };

    // 写 70
    let t0 = std::time::Instant::now();
    let ok70 = io.set(70);
    let write_ms = t0.elapsed().as_millis();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let readback = io.get();
    println!(
        "[probe] 写 70: ok={ok70} 写耗时={write_ms}ms 读回={readback:?} 吻合={}",
        readback == Some(70)
    );

    // 恢复原值
    let ok_restore = io.set(original);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let final_read = io.get();
    println!(
        "[probe] 恢复 {original}: ok={ok_restore} 读回={final_read:?} 吻合={}",
        final_read == Some(original)
    );

    // 断言（探针即验收：失败应显式报错，而不是静默通过）
    assert!(ok70, "写入 70 失败");
    assert_eq!(readback, Some(70), "读回值与写入不符");
    assert!(ok_restore, "恢复原值失败");
    assert_eq!(final_read, Some(original), "恢复后读回值与原始不符");
    println!("[probe] PASS：完整往返（读→写→读→恢复→读）验证通过");
}
