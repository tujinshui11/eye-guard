//! brightness（非 Windows 平台降级实现）
//!
//! macOS 真实现方向：IOKit `IODisplaySetFloatParameter`（内建屏可用；外接屏常无效）
//!
//! 当前为占位：背光始终"不可用"，亮度全部由黑纱（overlay）承担——
//! 即 Windows 上"台式机无背光"的等价降级路径，功能可运行。

use serde::Serialize;

/// 背光 IO 抽象（与 Windows 版同名，供 BrightnessController 泛型使用）
pub trait BacklightIo: Send {
    fn read(&mut self) -> Option<u32>;
    fn write(&mut self, percent: u32) -> bool;
}

/// 占位实现：读写均不支持
pub struct StubBacklight;

impl BacklightIo for StubBacklight {
    fn read(&mut self) -> Option<u32> {
        None
    }
    fn write(&mut self, _percent: u32) -> bool {
        false
    }
}

/// 亮度结果（与 Windows 版字段一致；此处只可能走 MaskOnly）
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BrightnessOutcome {
    BacklightOnly {
        backlight: u32,
    },
    BacklightPlusMask {
        backlight: u32,
        mask_alpha: f64,
    },
    MaskOnly {
        mask_alpha: f64,
    },
}

/// 亮度控制器（占位：背光不可用 → 全部黑纱）
pub struct BrightnessController<I: BacklightIo = StubBacklight> {
    io: I,
    available: bool,
    initial: Option<u32>,
}

impl BrightnessController<StubBacklight> {
    pub fn new() -> Self {
        Self {
            io: StubBacklight,
            available: false,
            initial: None,
        }
    }
}

impl Default for BrightnessController<StubBacklight> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: BacklightIo> BrightnessController<I> {
    pub fn with_io(io: I) -> Self {
        Self {
            io,
            available: false,
            initial: None,
        }
    }

    pub fn capture_initial(&mut self) {
        self.initial = self.io.read();
    }

    pub fn restore_initial(&mut self) -> bool {
        match self.initial {
            Some(v) => self.io.write(v),
            None => false,
        }
    }

    pub fn is_available(&mut self) -> bool {
        self.available = self.io.read().is_some();
        self.available
    }

    pub fn cached_available(&self) -> bool {
        self.available
    }

    pub fn current_backlight(&mut self) -> Option<u32> {
        self.io.read()
    }

    /// 与 Windows 版同语义：背光不可用 → 全部黑纱
    pub fn plan(&self, _target: u32, available: bool) -> (u32, f64) {
        if available {
            (100, 0.0)
        } else {
            (100, 0.0)
        }
    }

    pub fn apply(&mut self, _target: u32) -> (BrightnessOutcome, f64) {
        let alpha = 0.0;
        (BrightnessOutcome::MaskOnly { mask_alpha: alpha }, alpha)
    }

    pub fn detect_external_change(&self) -> Option<u32> {
        None
    }
}

/// 直接读取当前亮度（占位：不支持）
pub fn read_brightness() -> Option<u32> {
    None
}

/// 诊断读取（占位）
pub fn debug_read_brightness() -> String {
    "当前平台暂不支持背光读取".to_string()
}

/// 直接设置亮度（占位：失败）
pub fn set_brightness(_percent: u8) -> bool {
    false
}

/// 诊断设置（占位）
pub fn set_brightness_diag(_percent: u8) -> (bool, String) {
    (false, "当前平台暂不支持背光控制".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_controller_degrades_to_mask_only() {
        let mut c = BrightnessController::new();
        assert!(!c.is_available(), "占位实现应报告背光不可用");
        let (outcome, alpha) = c.apply(50);
        assert!(matches!(outcome, BrightnessOutcome::MaskOnly { .. }));
        assert_eq!(alpha, 0.0);
        assert!(read_brightness().is_none());
        assert!(!set_brightness(80));
    }
}
