//! 全局应用状态（settings + display + 各运行时）——对照 Electron 版主进程模块级单例

use crate::ambient::analyzer::LumaAnalyzer;
use crate::break_timer::BreakTimer;
use crate::brightness::{BrightnessController, WmiBacklight};
use crate::display::{DisplayController, RealGammaIo};
use crate::settings::SettingsStore;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

/// 建议卡片运行时（payload + 世代计数防重）
#[derive(Default)]
pub struct ProposeRuntime {
    pub payload: Option<Value>,
    pub generation: u64,
}

/// 感光监测运行时
#[derive(Default)]
pub struct AmbientRuntime {
    pub running: bool,
    pub source: Option<String>, // "als" | "camera"
    pub failures: u32,
    pub stopped_reason: Option<String>,
    pub analyzer: Option<LumaAnalyzer>,
    pub interval_ms: u64,
    pub session: u64,
}

pub struct AppState {
    pub settings: SettingsStore,
    pub display: DisplayController,
    pub propose: ProposeRuntime,
    pub ambient: AmbientRuntime,
    pub break_timer: BreakTimer,
    pub overlay_brightness: u32,
    /// C 方案亮度控制器（背光主控 + 黑纱补充）
    pub brightness: BrightnessController<WmiBacklight>,
    /// 色彩敏感应用挂起标记（Some=挂起中，存应用名；仅内存，不写 settings）
    pub app_suspended: Option<String>,
}

pub type SharedState = Mutex<AppState>;

/// 数据目录：沿用 Electron 版 userData 路径（%APPDATA%\护眼助手\）——设置与备份无缝继承
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("护眼助手")
}

/// 启动引导：加载设置 → 初始化 display（dirty 自愈）→ 应用启动状态（silent，不落盘）
pub fn bootstrap() -> AppState {
    let dir = data_dir();
    let mut settings = SettingsStore::new(&dir);
    settings.load();
    eprintln!(
        "[boot] settings loaded from {} (settings.json exists={})",
        dir.display(),
        dir.join("settings.json").exists()
    );

    let mut display = DisplayController::new(&dir, RealGammaIo);
    match display.init() {
        Ok(healed) => eprintln!("[boot] display initialized, healed={healed}"),
        Err(e) => eprintln!("[boot] display init failed: {e}"),
    }
    display.enabled = settings.get()["enabled"].as_bool().unwrap_or(true);

    // applyStartupState（对照 index.js:689）：模式命中走 applyMode 语义，silent 恢复
    if display.enabled {
        let mode_id = settings
            .get()
            .get("modeId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(mode) = crate::modes::get_mode(&mode_id) {
            let outcome = display.apply(mode.kelvin as f64);
            eprintln!(
                "[boot] 恢复模式 {}（{}）→ 生效 {}K / {}%",
                mode_id, mode.name, outcome.effective_temperature, mode.brightness
            );
        } else if let Some(t) = settings.get().get("temperature").and_then(|v| v.as_f64()) {
            if (t - 6500.0).abs() > f64::EPSILON {
                let outcome = display.apply(t);
                eprintln!("[boot] 恢复上次色温: {}K", outcome.effective_temperature);
            }
        }
    }

    AppState {
        settings,
        display,
        propose: ProposeRuntime::default(),
        ambient: AmbientRuntime::default(),
        break_timer: BreakTimer::new(),
        overlay_brightness: 100,
        brightness: BrightnessController::new(WmiBacklight),
        app_suspended: None,
    }
}

/// display:changed 的广播载荷（对照 broadcastDisplayChanged）
pub fn display_payload(state: &AppState) -> Value {
    let s = state.settings.get();
    json!({
        "temperature": state.display.current_temperature,
        "brightness": state.overlay_brightness,
        "modeId": s.get("modeId").cloned().unwrap_or(json!("natural")),
        "enabled": s.get("enabled").cloned().unwrap_or(json!(true))
    })
}
