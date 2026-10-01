//! DisplayController：屏幕色彩状态控制——对照移植 `src/main/display.js`
//!
//! 职责：
//! - 启动时存档原始 gamma ramp（内存 + 磁盘 gamma-backup.json 双备份）
//! - apply：应用色温（安全钳制 + 写入失败夹逼回退 + 持久失败回退上一有效状态）
//! - restore：恢复原始色彩
//! - dirty 标记：防崩溃后屏幕偏色滞留（下次启动自愈）
//!
//! 亮度调节不在本模块（透明遮罩实现——不占用 gamma 空间）。

use crate::gamma;
use crate::temperature::{build_safe_lut, build_safe_lut_with_max, SAFE_MAX_VALUE};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// gamma IO 抽象（测试注入 Fake；默认真实 gdi32）
pub trait GammaIo {
    fn read_ramp(&mut self) -> Result<[u16; 768], String>;
    fn write_ramp(&mut self, lut: &[u16; 768]) -> bool;
}

/// 真实 gamma IO（gdi32）
pub struct RealGammaIo;

impl GammaIo for RealGammaIo {
    fn read_ramp(&mut self) -> Result<[u16; 768], String> {
        gamma::read_ramp()
    }
    fn write_ramp(&mut self, lut: &[u16; 768]) -> bool {
        gamma::write_ramp(lut)
    }
}

/// apply 的结果（对照 JS 返回对象）
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub ok: bool,
    pub clamped: bool,
    pub effective_temperature: f64,
    pub skipped: bool,
    pub error: Option<String>,
}

/// 显示控制器
pub struct DisplayController {
    #[allow(dead_code)]
    data_dir: PathBuf,
    backup_path: PathBuf,
    gamma: Box<dyn GammaIo>,
    original_ramp: Option<[u16; 768]>,
    last_good_lut: Option<[u16; 768]>,
    dirty: bool,
    /// 当前生效色温（UI 读取）
    pub current_temperature: f64,
    /// 启用标志（关闭时 apply 直接跳过）
    pub enabled: bool,
}

impl DisplayController {
    pub fn new(data_dir: &Path, gamma_io: Box<dyn GammaIo>) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            backup_path: data_dir.join("gamma-backup.json"),
            gamma: gamma_io,
            original_ramp: None,
            last_good_lut: None,
            dirty: false,
            current_temperature: 6500.0,
            enabled: true,
        }
    }

    /// 启动初始化：
    ///  有备份（dirty=true）→ 先恢复原始色彩（自愈）→ 再继续
    ///  无备份 → 读取当前 ramp 存档
    /// @returns healed：是否执行了自愈恢复
    pub fn init(&mut self) -> Result<bool, String> {
        let backup = self.read_backup();
        let mut healed = false;

        if let Some(ramp) = backup.as_ref().and_then(valid_ramp_from_backup) {
            self.original_ramp = Some(ramp);
            self.dirty = backup
                .as_ref()
                .and_then(|b| b.get("dirty"))
                .and_then(|d| d.as_bool())
                .unwrap_or(false);
            if self.dirty {
                let ok = self.gamma.write_ramp(&ramp);
                healed = ok;
                eprintln!(
                    "[display] 检测到上次异常退出（dirty=true），恢复原始色彩：{}",
                    if ok { "OK" } else { "FAILED" }
                );
            }
            self.set_dirty(false);
        } else {
            let ramp = self.gamma.read_ramp()?;
            self.original_ramp = Some(ramp);
            self.dirty = false;
            self.write_backup(false);
            eprintln!("[display] 首次运行：已存档原始 ramp");
        }
        Ok(healed)
    }

    /// 应用色温（亮度固定 100%——亮度由遮罩层负责）
    pub fn apply(&mut self, temperature: f64) -> ApplyOutcome {
        let Some(original) = self.original_ramp else {
            return ApplyOutcome {
                ok: false,
                clamped: false,
                effective_temperature: 6500.0,
                skipped: false,
                error: Some("DisplayController 未初始化（先调用 init）".to_string()),
            };
        };
        if !self.enabled {
            return ApplyOutcome {
                ok: true,
                clamped: false,
                effective_temperature: 6500.0,
                skipped: true,
                error: None,
            };
        }

        // 应用前先落 dirty——崩溃后下次启动才能自愈
        self.set_dirty(true);

        let mut safe: u32 = SAFE_MAX_VALUE as u32;
        let mut result = build_safe_lut(&original, temperature, 100.0);
        let mut ok = self.gamma.write_ramp(&result.lut);

        // 夹逼回退：面向阈值比 32768 更严的驱动（每次提升安全边界重试）
        let mut i = 0;
        while !ok && i < 8 {
            safe += 4096;
            if safe > 65535 {
                break;
            }
            result = build_safe_lut_with_max(&original, temperature, 100.0, safe as u16);
            ok = self.gamma.write_ramp(&result.lut);
            i += 1;
        }

        if !ok {
            // 彻底失败：回退上一有效状态（或原始值）
            let fallback = self.last_good_lut.unwrap_or(original);
            let _ = self.gamma.write_ramp(&fallback);
            return ApplyOutcome {
                ok: false,
                clamped: true,
                effective_temperature: self.current_temperature,
                skipped: false,
                error: Some("该色温超出当前显卡驱动的支持范围".to_string()),
            };
        }

        self.last_good_lut = Some(result.lut);
        self.current_temperature = result.effective_temperature;
        ApplyOutcome {
            ok: true,
            clamped: result.clamped,
            effective_temperature: result.effective_temperature,
            skipped: false,
            error: None,
        }
    }

    /// 恢复原始色彩（并清除 dirty）
    pub fn restore(&mut self) -> bool {
        let Some(original) = self.original_ramp else {
            return false;
        };
        let ok = self.gamma.write_ramp(&original);
        self.set_dirty(!ok); // 失败则保持 dirty，等下次启动自愈
        ok
    }

    /// 提取当前原始 ramp 副本（selftest/调试用）
    pub fn get_original_ramp(&self) -> Option<[u16; 768]> {
        self.original_ramp
    }

    // ---- 备份文件 ----

    fn read_backup(&self) -> Option<Value> {
        let text = fs::read_to_string(&self.backup_path).ok()?;
        serde_json::from_str::<Value>(&text).ok()
    }

    fn set_dirty(&mut self, dirty: bool) {
        if self.dirty == dirty {
            return; // 状态未变不写盘（拖滑块高频调用时的节流）
        }
        self.dirty = dirty;
        self.write_backup(dirty);
    }

    fn write_backup(&self, dirty: bool) {
        let Some(ramp) = self.original_ramp else { return };
        let saved_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        // 偏离记录：savedAt 由 JS 版 ISO 字符串改为 unix_ms（该字段无消费方，仅记录）
        let payload = json!({
            "savedAt": saved_at,
            "dirty": dirty,
            "ramp": ramp.to_vec()
        });
        if let Some(dir) = self.backup_path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Err(e) = fs::write(&self.backup_path, payload.to_string()) {
            eprintln!("[display] 写入备份失败: {e}");
        }
    }
}

/// 备份中的 ramp 有效性检查（长度必须为 768；无效返回 None）
fn valid_ramp_from_backup(v: &Value) -> Option<[u16; 768]> {
    let arr = v.get("ramp")?.as_array()?;
    if arr.len() != 768 {
        return None;
    }
    let mut out = [0u16; 768];
    for (i, item) in arr.iter().enumerate() {
        out[i] = item.as_u64()? as u16;
    }
    Some(out)
}

// ============================================================
// 测试——对照移植 tests/backup.test.js（7 用例）
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn std_ramp() -> [u16; 768] {
        let mut r = [0u16; 768];
        for (i, v) in r.iter_mut().enumerate() {
            *v = ((i % 256) * 257) as u16;
        }
        r
    }

    /// 共享句柄 fake：测试可随时检视/操控设备状态
    struct FakeState {
        ramp: [u16; 768],
        writes: Vec<[u16; 768]>,
        fail_all: bool,
    }

    struct FakeGamma {
        state: Rc<RefCell<FakeState>>,
    }

    impl FakeGamma {
        fn new(initial: [u16; 768]) -> (Self, Rc<RefCell<FakeState>>) {
            let state = Rc::new(RefCell::new(FakeState {
                ramp: initial,
                writes: Vec::new(),
                fail_all: false,
            }));
            (Self { state: state.clone() }, state)
        }
    }

    impl GammaIo for FakeGamma {
        fn read_ramp(&mut self) -> Result<[u16; 768], String> {
            Ok(self.state.borrow().ramp)
        }
        fn write_ramp(&mut self, lut: &[u16; 768]) -> bool {
            let mut s = self.state.borrow_mut();
            if s.fail_all {
                return false;
            }
            s.ramp = *lut;
            s.writes.push(*lut);
            true
        }
    }

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("eyeguard-display-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_dc(
        tag: &str,
        initial: [u16; 768],
    ) -> (DisplayController, PathBuf, Rc<RefCell<FakeState>>) {
        let dir = tmp_dir(tag);
        let (fake, state) = FakeGamma::new(initial);
        (DisplayController::new(&dir, Box::new(fake)), dir, state)
    }

    fn read_backup(dir: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(dir.join("gamma-backup.json")).unwrap()).unwrap()
    }

    // 'init: 首次运行——存档原始 ramp 并创建备份（dirty=false）'
    #[test]
    fn init_first_run_archives() {
        let (mut dc, dir, _state) = make_dc("first", std_ramp());
        let healed = dc.init().unwrap();
        assert!(!healed);

        let backup = read_backup(&dir);
        assert_eq!(backup["dirty"], false);
        assert_eq!(backup["ramp"].as_array().unwrap().len(), 768);
        assert_eq!(backup["ramp"][255], 65535); // (255%256)*257 = 65535
    }

    // 'init: dirty=true 的备份触发自愈——先恢复原始色彩'
    #[test]
    fn init_dirty_heals() {
        let dir = tmp_dir("heal");
        let original = std_ramp();
        let biased = [65535u16; 768]; // 模拟"上次偏色未恢复"
        fs::write(
            dir.join("gamma-backup.json"),
            json!({ "savedAt": 0, "dirty": true, "ramp": original.to_vec() }).to_string(),
        )
        .unwrap();

        let (fake, state) = FakeGamma::new(biased);
        let mut dc = DisplayController::new(&dir, Box::new(fake));
        let healed = dc.init().unwrap();

        assert!(healed);
        assert_eq!(state.borrow().ramp, original, "屏幕应已被恢复为原始值");
        assert_eq!(read_backup(&dir)["dirty"], false);
    }

    // 'init: dirty=false 的备份不做屏幕写入'
    #[test]
    fn init_clean_backup_no_write() {
        let dir = tmp_dir("clean");
        fs::write(
            dir.join("gamma-backup.json"),
            json!({ "savedAt": 0, "dirty": false, "ramp": std_ramp().to_vec() }).to_string(),
        )
        .unwrap();
        let (fake, state) = FakeGamma::new(std_ramp());
        let mut dc = DisplayController::new(&dir, Box::new(fake));
        dc.init().unwrap();
        assert!(
            state.borrow().writes.is_empty(),
            "dirty=false 不应产生任何屏幕写入"
        );
    }

    // 'apply: 应用色温——LUT 写入设备、备份转 dirty'
    #[test]
    fn apply_writes_and_marks_dirty() {
        let (mut dc, dir, state) = make_dc("apply", std_ramp());
        dc.init().unwrap();
        let res = dc.apply(4500.0);
        assert!(res.ok);
        assert!(!res.clamped);
        assert_eq!(state.borrow().writes.len(), 1);

        assert_eq!(read_backup(&dir)["dirty"], true);
    }

    // 'apply: 越界色温（2700K）自动钳制并回报生效色温'
    #[test]
    fn apply_2700_clamped() {
        let (mut dc, _dir, state) = make_dc("clamp", std_ramp());
        dc.init().unwrap();
        let res = dc.apply(2700.0);
        assert!(res.ok);
        assert!(res.clamped);
        assert!(
            res.effective_temperature > 2700.0,
            "生效 {}",
            res.effective_temperature
        );
        assert!(res.effective_temperature <= 3400.0);
        let b_max = *state.borrow().ramp[512..].iter().max().unwrap();
        assert!(b_max >= 32768, "B max={b_max}");
    }

    // 'restore: 恢复原始色彩并清除 dirty'
    #[test]
    fn restore_resets_and_clears_dirty() {
        let (mut dc, dir, state) = make_dc("restore", std_ramp());
        dc.init().unwrap();
        dc.apply(4500.0);
        let ok = dc.restore();
        assert!(ok);
        assert_eq!(state.borrow().ramp, std_ramp(), "屏幕应已恢复原始值");
        assert_eq!(read_backup(&dir)["dirty"], false);
    }

    // 'apply: 设备持续拒绝时回退并返回 ok=false + error'
    #[test]
    fn apply_device_rejects_falls_back() {
        let (mut dc, _dir, state) = make_dc("reject", std_ramp());
        dc.init().unwrap();
        let first = dc.apply(4500.0); // 先建立"上一有效状态"
        assert!(first.ok);

        state.borrow_mut().fail_all = true; // 设备从此持续拒绝
        let res = dc.apply(3400.0);
        assert!(!res.ok);
        assert!(res.error.is_some());
    }
}