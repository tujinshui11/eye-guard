//! Tauri IPC 命令——对照移植 src/main/index.js 的 ipcMain.handle 区（全量）

use crate::display::ApplyOutcome;
use crate::modes::{get_mode, MODES};
use crate::state::{display_payload, SharedState};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

fn outcome_to_json(o: &ApplyOutcome) -> Value {
    let mut v = json!({
        "ok": o.ok,
        "clamped": o.clamped,
        "effectiveTemperature": o.effective_temperature
    });
    if o.skipped {
        v["skipped"] = json!(true);
    }
    if let Some(e) = &o.error {
        v["error"] = json!(e);
    }
    v
}

/// applyMode 的单一入口内部实现（对照 index.js:176）——命令与调度/感光/托盘共用
pub(crate) fn apply_mode_with_app(
    app: &AppHandle,
    mode_id: &str,
    persist: bool,
    silent: bool,
) -> Value {
    let state = app.state::<SharedState>();

    // 阶段 1（持锁）：色温 + 亮度（C 方案：背光主控 + 黑纱补充）
    let (mode_name, mode_kelvin, mode_brightness, outcome, effective, mask_alpha) = {
        let mut st = state.lock().unwrap();
        let Some(mode) = get_mode(mode_id) else {
            return json!({ "ok": false, "error": format!("未知模式：{mode_id}") });
        };
        let outcome = st.display.apply(mode.kelvin as f64);
        let effective = st.display.current_temperature;
        let (_b_outcome, alpha) = st.brightness.apply(mode.brightness);
        st.overlay_brightness = mode.brightness;
        (
            mode.name,
            mode.kelvin,
            mode.brightness,
            outcome,
            effective,
            alpha,
        )
    };

    // 阶段 2（放锁）：黑纱窗口操作（内部会再取锁，必须在外层锁释放后调用）
    crate::overlay::apply_mask_alpha(app, mask_alpha);

    // 阶段 3（持锁）：持久化 + 广播
    {
        let mut st = state.lock().unwrap();
        if persist {
            if let Err(e) = st.settings.save(&json!({
                "enabled": true,
                "modeId": mode_id,
                "temperature": effective,
                "brightness": mode_brightness
            })) {
                return json!({ "ok": false, "error": e.to_string() });
            }
        }
        if !silent {
            let _ = app.emit("display:changed", display_payload(&st));
        }
    }

    json!({
        "ok": outcome.ok,
        "mode": {
            "id": mode_id,
            "name": mode_name,
            "kelvin": mode_kelvin,
            "brightness": mode_brightness
        },
        "temperature": effective,
        "brightness": mode_brightness
    })
}

/// setTemperature 的内部实现（对照 index.js:118）——命令 / 热键 / 托盘微调共用
pub(crate) fn set_temperature_with_app(app: &AppHandle, kelvin: f64, mode_id: &str) -> Value {
    let state = app.state::<SharedState>();
    let mut st = state.lock().unwrap();
    let n = if kelvin == 0.0 || kelvin.is_nan() {
        6500.0
    } else {
        kelvin
    };
    let t = n.clamp(2000.0, 10000.0);
    let outcome = st.display.apply(t);
    if outcome.ok {
        if let Err(e) = st.settings.save(&json!({
            "temperature": outcome.effective_temperature,
            "modeId": mode_id
        })) {
            return json!({ "ok": false, "error": e.to_string() });
        }
    }
    if outcome.ok {
        let _ = app.emit("display:changed", display_payload(&st));
    }
    outcome_to_json(&outcome)
}

/// 色温微调（对照 nudgeTemperature）：current ± delta → clamp → setTemperature
pub(crate) fn nudge_temperature_with_app(app: &AppHandle, delta: f64) {
    let cur = {
        let state = app.state::<SharedState>();
        let st = state.lock().unwrap();
        st.display.current_temperature
    };
    let next = (cur + delta).clamp(2000.0, 10000.0);
    set_temperature_with_app(app, next, "custom");
}

/// 恢复原色（对照 restoreColor）：先落 enabled=false → 恢复原始色 → 广播 → 返回 ok
///
/// C 方案：同时还原系统背光到启动原值 + 隐藏黑纱（「恢复原色」语义 = 屏幕完全交还用户）。
/// 注意：apply_mask_alpha 会触达 overlay 窗口（内部回调取同一把锁），必须在放锁后调用。
pub(crate) fn restore_color_with_app(app: &AppHandle) -> bool {
    let (ok, payload) = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let _ = st.settings.save(&json!({ "enabled": false }));
        let ok = st.display.restore();
        // C 方案：还原系统背光到启动原值
        let _ = st.brightness.restore_initial();
        st.overlay_brightness = 100;
        (ok, display_payload(&st))
    };
    // 放锁后：隐藏黑纱 + 广播
    crate::overlay::apply_mask_alpha(app, 0.0);
    let _ = app.emit("display:changed", payload);
    ok
}

/// 重新启用（对照 reenableColor）：save enabled:true → 模式命中 applyMode / 否则恢复上次色温
///
/// C 方案：重新启用时同时恢复上次亮度设置（否则「恢复原色」还原了背光，「重新启用」
/// 只回色温、背光停原值，与设置中的 brightness 不一致）。
pub(crate) fn reenable_color_with_app(app: &AppHandle) {
    let (mode_id, temperature, brightness) = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let _ = st.settings.save(&json!({ "enabled": true }));
        let s = st.settings.get().clone();
        (
            s.get("modeId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            s.get("temperature").and_then(|v| v.as_f64()).unwrap_or(4500.0),
            s.get("brightness").and_then(|v| v.as_u64()).unwrap_or(100) as u32,
        )
    };
    if get_mode(&mode_id).is_some() {
        apply_mode_with_app(app, &mode_id, true, false);
    } else {
        set_temperature_with_app(app, temperature, "custom");
        // 自定义路径：补应用亮度（模式路径已由 apply_mode_with_app 处理）
        let (outcome, alpha) = {
            let state = app.state::<SharedState>();
            let mut st = state.lock().unwrap();
            let (outcome, alpha) = st.brightness.apply(brightness);
            st.overlay_brightness = brightness;
            (outcome, alpha)
        };
        let _ = outcome;
        crate::overlay::apply_mask_alpha(app, alpha);
    }
}

// ============================ commands ============================

#[tauri::command]
pub fn app_get_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// 对照 display:get-state（index.js:718）
#[tauri::command]
pub fn display_get_state(state: State<'_, SharedState>) -> Value {
    let st = state.lock().unwrap();
    let s = st.settings.get();
    json!({
        "available": true,
        "enabled": s.get("enabled").cloned().unwrap_or(json!(true)),
        "temperature": st.display.current_temperature,
        "brightness": st.overlay_brightness,
        "modeId": s.get("modeId").cloned().unwrap_or(json!("natural"))
    })
}

/// 对照 display:set-temperature（index.js:712）
///
/// 【v0.2.2 修复】改为 async：Tauri 同步命令跑在主线程，WMI 写入（数十毫秒）
/// 会阻塞 UI；async 命令调度到线程池执行。
#[tauri::command]
pub async fn display_set_temperature(app: AppHandle, kelvin: f64) -> Value {
    set_temperature_with_app(&app, kelvin, "custom")
}

/// 对照 display:set-brightness（index.js:728）→ C 方案：背光主控 + 黑纱补充
///
/// 【v0.2.2 修复】async 化：拖滑块时每 60ms 一次 WMI 写入，同步命令会反复
/// 占用主线程导致界面卡顿——异步化后 WMI 调用在线程池执行。
#[tauri::command]
pub async fn display_set_brightness(app: AppHandle, brightness: f64) -> Value {
    let target = brightness.clamp(0.0, 100.0) as u32;
    let (outcome, alpha) = {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let (outcome, alpha) = st.brightness.apply(target);
        st.overlay_brightness = target;
        (outcome, alpha)
    };
    // 黑纱层：由控制器算出的 alpha 驱动（背光承担主量，遮罩只补差额）
    crate::overlay::apply_mask_alpha(&app, alpha);
    {
        let state = app.state::<SharedState>();
        let mut st = state.lock().unwrap();
        let _ = st.settings.save(&json!({ "brightness": target, "modeId": "custom" }));
        let _ = app.emit("display:changed", display_payload(&st));
    }
    let mut res = json!({ "ok": true, "brightness": target });
    match outcome {
        crate::brightness::BrightnessOutcome::BacklightOnly { backlight } => {
            res["backlight"] = json!(backlight);
        }
        crate::brightness::BrightnessOutcome::BacklightPlusMask { backlight, mask_alpha } => {
            res["backlight"] = json!(backlight);
            res["maskAlpha"] = json!(mask_alpha);
        }
        crate::brightness::BrightnessOutcome::MaskOnly { mask_alpha } => {
            res["maskAlpha"] = json!(mask_alpha);
        }
    }
    res
}

/// 对照 display:restore（index.js:726）—— 恢复原色（含 WMI 背光还原，async 避免阻塞）
#[tauri::command]
pub async fn display_restore(app: AppHandle) -> bool {
    restore_color_with_app(&app)
}

/// 对照 modes:list（index.js:714）
#[tauri::command]
pub fn modes_list() -> Vec<Value> {    MODES
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "name": m.name,
                "kelvin": m.kelvin,
                "brightness": m.brightness
            })
        })
        .collect()
}

/// 对照 modes:apply —— 切换模式（含 WMI 背光写入，async 避免阻塞主线程）
#[tauri::command]
pub async fn modes_apply(app: AppHandle, mode_id: String) -> Value {
    apply_mode_with_app(&app, &mode_id, true, false)
}

/// 对照 settings:get（index.js:730）
#[tauri::command]
pub fn settings_get(state: State<'_, SharedState>) -> Value {
    state.lock().unwrap().settings.get().clone()
}

/// 对照 settings:set（index.js:732）：clean patch → save → theme/ambient/breaks 联动 → 返回 next
#[tauri::command]
pub fn settings_set(
    app: AppHandle,
    state: State<'_, SharedState>,
    patch: Value,
) -> Result<Value, String> {
    let clean = if patch.is_object() { patch } else { json!({}) };
    let next = {
        let mut st = state.lock().unwrap();
        st.settings
            .save(&clean)
            .map_err(|e| e.to_string())?
            .clone()
    };

    if clean.get("theme").is_some() {
        let theme = next.get("theme").cloned().unwrap_or(json!("dark"));
        let _ = app.emit("theme:changed", theme);
    }
    if clean.get("ambient").is_some() {
        crate::ambient::monitor::start_ambient_monitor(&app, "settings");
    }
    if clean.get("breaks").is_some() {
        // 对照 JS breaks 联动：重配置 + 启停
        let breaks = next.get("breaks").cloned().unwrap_or(json!({}));
        let enabled = breaks.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let now = now_ms();
        {
            let state = app.state::<SharedState>();
            let mut st = state.lock().unwrap();
            st.break_timer.configure(
                breaks.get("workSeconds").and_then(|v| v.as_u64()),
                breaks.get("breakSeconds").and_then(|v| v.as_u64()),
                now,
            );
            if enabled {
                if st.break_timer.get_state(now).state == crate::break_timer::BreakState::Idle {
                    st.break_timer.start(now);
                }
            } else {
                st.break_timer.stop();
            }
        }
        if !enabled {
            crate::break_rt::hide_break_window(&app);
        }
        eprintln!("[break] 配置已更新并联动");
    }

    Ok(next)
}

// ---- 休息提醒 ----

/// 对照 break:get-state（index.js:774）
#[tauri::command]
pub fn break_get_state(state: State<'_, SharedState>) -> Value {
    let st = state.lock().unwrap();
    let snap = st.break_timer.get_state(now_ms());
    json!({
        "state": snap.state.as_str(),
        "remainingSeconds": snap.remaining_seconds,
        "pausedUntil": snap.paused_until
    })
}

/// 对照 break:action（index.js:776）
#[tauri::command]
pub fn break_action(app: AppHandle, name: String) -> bool {
    let now = now_ms();
    match name.as_str() {
        "beginRest" => {
            let ok = {
                let state = app.state::<SharedState>();
                let mut st = state.lock().unwrap();
                st.break_timer.begin_rest(now)
            };
            if ok {
                crate::break_rt::push_break_update(&app); // 即时切换"休息中"UI
            }
            ok
        }
        "postpone" => {
            let ok = {
                let state = app.state::<SharedState>();
                let mut st = state.lock().unwrap();
                st.break_timer.postpone(now)
            };
            if ok {
                crate::break_rt::hide_break_window(&app); // 即时关窗（v0.2.1：修复关不掉）
            }
            ok
        }
        "skip" => {
            let ok = {
                let state = app.state::<SharedState>();
                let mut st = state.lock().unwrap();
                st.break_timer.skip(now)
            };
            if ok {
                crate::break_rt::hide_break_window(&app); // 即时关窗（v0.2.1：修复关不掉）
            }
            ok
        }
        "pause1h" => crate::break_rt::pause_breaks(&app, 1),
        "resume" => crate::break_rt::resume_breaks(&app),
        _ => false,
    }
}

// ---- 开机自启（对照 index.js:211 / :762 / :764）----

#[tauri::command]
pub fn app_set_auto_launch(app: AppHandle, enabled: bool) -> Value {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    let result = if enabled { al.enable() } else { al.disable() };
    match result {
        Ok(_) => {
            let open_at_login = al.is_enabled().unwrap_or(false);
            {
                let state = app.state::<SharedState>();
                let mut st = state.lock().unwrap();
                let _ = st.settings.save(&json!({ "autoLaunch": enabled }));
            }
            eprintln!("[autoLaunch] 设置 openAtLogin={enabled} | 回读={open_at_login}");
            json!({ "ok": true, "openAtLogin": open_at_login, "args": ["--hidden"] })
        }
        Err(e) => {
            eprintln!("[autoLaunch] 设置失败: {e}");
            json!({ "ok": false, "error": e.to_string() })
        }
    }
}

#[tauri::command]
pub fn app_get_auto_launch(app: AppHandle) -> Value {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    match al.is_enabled() {
        Ok(on) => json!({ "openAtLogin": on, "args": ["--hidden"] }),
        Err(e) => json!({ "openAtLogin": false, "args": [], "error": e.to_string() }),
    }
}

// ---- 建议卡片 / 感光 ----

/// 对照 propose:action（index.js:794）
#[tauri::command]
pub fn propose_action(app: AppHandle, name: String) -> bool {
    crate::propose::resolve_propose(&app, &name)
}

/// 对照 ambient:get-state（index.js:799）
#[tauri::command]
pub fn ambient_get_state(app: AppHandle) -> Value {
    crate::ambient::monitor::ambient_state(&app)
}

// ---- 已安装应用扫描（色彩敏感应用选择器）----

/// 扫描开始菜单已安装应用（供「色彩敏感应用」搜索选择器）
///
/// async：全量扫描 + COM 解析耗时数百毫秒，避免阻塞主线程。
#[tauri::command]
pub async fn apps_list_installed() -> Vec<crate::appscan::AppEntry> {
    crate::appscan::scan_installed_apps()
}
