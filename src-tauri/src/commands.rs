//! Tauri IPC 命令——对照移植 src/main/index.js 的 ipcMain.handle 区
//!
//! 已覆盖：W2 显示类 8 命令 + W3 propose_action / ambient_get_state
//! 延后：display_set_brightness（与 overlay 遮罩同属 W4——JS 版亮度即遮罩驱动）

use crate::display::ApplyOutcome;
use crate::modes::{get_mode, MODES};
use crate::state::{display_payload, AppState, SharedState};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

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

/// applyMode 的单一入口内部实现（对照 index.js:176）——命令与调度/感光壳共用
pub(crate) fn apply_mode_with_app(
    app: &AppHandle,
    mode_id: &str,
    persist: bool,
    silent: bool,
) -> Value {
    let state = app.state::<SharedState>();
    let mut st = state.lock().unwrap();
    let Some(mode) = get_mode(mode_id) else {
        return json!({ "ok": false, "error": format!("未知模式：{mode_id}") });
    };

    let outcome = st.display.apply(mode.kelvin as f64);
    let effective = st.display.current_temperature;

    if persist {
        if let Err(e) = st.settings.save(&json!({
            "enabled": true,
            "modeId": mode.id,
            "temperature": effective,
            "brightness": mode.brightness
        })) {
            return json!({ "ok": false, "error": e.to_string() });
        }
    }

    if !silent {
        let _ = app.emit("display:changed", display_payload(&st));
    }

    json!({
        "ok": outcome.ok,
        "mode": {
            "id": mode.id,
            "name": mode.name,
            "kelvin": mode.kelvin,
            "brightness": mode.brightness
        },
        "temperature": effective,
        "brightness": mode.brightness
    })
}

#[tauri::command]
pub fn app_get_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// 对照 display:get-state（index.js:718）——brightness 为 100 占位（W4 overlay 接入后改读实际值）
#[tauri::command]
pub fn display_get_state(state: State<'_, SharedState>) -> Value {
    let st = state.lock().unwrap();
    let s = st.settings.get();
    json!({
        "available": true,
        "enabled": s.get("enabled").cloned().unwrap_or(json!(true)),
        "temperature": st.display.current_temperature,
        "brightness": 100,
        "modeId": s.get("modeId").cloned().unwrap_or(json!("natural"))
    })
}

/// 对照 setTemperature（index.js:118）：clamp(2000,10000)（0/NaN→6500）→ apply →
/// 成功才持久化生效色温（modeId=custom）+ 成功才广播
#[tauri::command]
pub fn display_set_temperature(
    app: AppHandle,
    state: State<'_, SharedState>,
    kelvin: f64,
) -> Result<Value, String> {
    let mut st = state.lock().unwrap();
    let n = if kelvin == 0.0 || kelvin.is_nan() {
        6500.0
    } else {
        kelvin
    };
    let t = n.clamp(2000.0, 10000.0);
    let outcome = st.display.apply(t);
    if outcome.ok {
        st.settings
            .save(&json!({
                "temperature": outcome.effective_temperature,
                "modeId": "custom"
            }))
            .map_err(|e| e.to_string())?;
    }
    if outcome.ok {
        let _ = app.emit("display:changed", display_payload(&st));
    }
    Ok(outcome_to_json(&outcome))
}

/// 对照 restoreColor（index.js:151）：先落 enabled=false → 恢复原始色 → 广播 → 返回 ok
#[tauri::command]
pub fn display_restore(app: AppHandle, state: State<'_, SharedState>) -> Result<bool, String> {
    let mut st = state.lock().unwrap();
    st.settings
        .save(&json!({ "enabled": false }))
        .map_err(|e| e.to_string())?;
    let ok = st.display.restore();
    let _ = app.emit("display:changed", display_payload(&st));
    Ok(ok)
}

/// 对照 modes:list（index.js:714）
#[tauri::command]
pub fn modes_list() -> Vec<Value> {
    MODES
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

/// 对照 modes:apply（index.js:716）——默认 persist=true、silent=false
#[tauri::command]
pub fn modes_apply(app: AppHandle, mode_id: String) -> Value {
    apply_mode_with_app(&app, &mode_id, true, false)
}

/// 对照 settings:get（index.js:730）
#[tauri::command]
pub fn settings_get(state: State<'_, SharedState>) -> Value {
    state.lock().unwrap().settings.get().clone()
}

/// 对照 settings:set（index.js:732）：clean patch → save → theme/ambient 联动 → 返回 next
/// （breaks 联动属 W4）
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
        let theme = next.get("theme").cloned().unwrap_or(json!("deepsea"));
        let _ = app.emit("theme:changed", theme);
    }
    if clean.get("ambient").is_some() {
        crate::ambient::monitor::start_ambient_monitor(&app, "settings");
    }

    Ok(next)
}

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

/// 供状态读取（未导出至 IPC 的内部使用）
#[allow(dead_code)]
fn _state_typecheck(_: &AppState) {}
