//! 感光监测运行时壳——对照 index.js 的 startAmbientMonitor / sampleAmbientOnce / handleAmbientDarker

use crate::ambient::{
    als,
    analyzer::{AnalyzerOptions, LumaAnalyzer},
    camera,
};
use crate::state::SharedState;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// 采样循环世代（每次 start 递增，使在途循环失效）
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// getAmbientState 快照（对照 getAmbientState）
pub fn ambient_state(app: &AppHandle) -> serde_json::Value {
    let state = app.state::<SharedState>();
    let st = state.lock().unwrap();
    let enabled = st
        .settings
        .get()
        .get("ambient")
        .and_then(|a| a.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let analyzer = st.ambient.analyzer.as_ref().map(|a| {
        let s = a.snapshot();
        json!({
            "smooth": s.smooth,
            "baseline": s.baseline,
            "state": if s.dark { "dark" } else { "normal" },
            "samples": s.samples
        })
    });
    json!({
        "enabled": enabled,
        "running": st.ambient.running,
        "source": st.ambient.source,
        "failures": st.ambient.failures,
        "stoppedReason": st.ambient.stopped_reason,
        "analyzer": analyzer
    })
}

/// 广播 ambient:state
pub fn broadcast_ambient_state(app: &AppHandle) {
    let st = ambient_state(app);
    let _ = app.emit("ambient:state", st);
}

/// 停止监测（对照 stopAmbientMonitor）
pub fn stop_ambient_monitor(app: &AppHandle, reason: Option<&str>) {
    GENERATION.fetch_add(1, Ordering::SeqCst); // 使在途采样循环失效
    let state = app.state::<SharedState>();
    {
        let mut st = state.lock().unwrap();
        st.ambient.running = false;
        st.ambient.source = None;
        st.ambient.session += 1;
        if let Some(r) = reason {
            st.ambient.stopped_reason = Some(r.to_string());
        }
    }
    broadcast_ambient_state(app);
    if let Some(r) = reason {
        eprintln!("[ambient] monitor stopped: {r}");
    }
}

/// 启动监测（对照 startAmbientMonitor）：先停 → 读配置 → 探测 ALS → 起采样循环
pub fn start_ambient_monitor(app: &AppHandle, _reason: &str) {
    stop_ambient_monitor(app, None);

    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let state = app.state::<SharedState>();

    let (enabled, interval_ms, analyzer_opts) = {
        let st = state.lock().unwrap();
        let cfg = st.settings.get().get("ambient").cloned().unwrap_or(json!({}));
        let enabled = cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let interval_s = cfg
            .get("intervalSeconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(60)
            .clamp(30, 300);
        let opts = AnalyzerOptions {
            drop_threshold_percent: cfg
                .get("dropThresholdPercent")
                .and_then(|v| v.as_f64())
                .unwrap_or(35.0),
            cooldown_ms: cfg
                .get("cooldownMinutes")
                .and_then(|v| v.as_i64())
                .unwrap_or(15)
                * 60
                * 1000,
            ..AnalyzerOptions::default()
        };
        (enabled, interval_s * 1000, opts)
    };

    if !enabled {
        eprintln!("[ambient] 未启用（默认关闭）");
        return;
    }

    // ALS 优先：有硬件用传感器（免摄像头），无则回退摄像头（本机实测无 ALS）
    let has_als = als::detect_als();
    {
        let mut st = state.lock().unwrap();
        st.ambient.stopped_reason = None;
        st.ambient.failures = 0;
        st.ambient.analyzer = Some(LumaAnalyzer::new(analyzer_opts));
        st.ambient.running = true;
        st.ambient.interval_ms = interval_ms;
        st.ambient.source = Some(if has_als { "als".into() } else { "camera".into() });
    }
    let source = if has_als { "als" } else { "camera" };
    eprintln!("[ambient] monitor started, source={source}, interval={interval_ms}ms");
    broadcast_ambient_state(app);

    let app2 = app.clone();
    std::thread::spawn(move || {
        // 首次采样（Rust 直采样无窗口就绪问题；短延迟让启动界面先渲染）
        std::thread::sleep(Duration::from_millis(200));
        if !sample_if_current(&app2, generation) {
            return;
        }
        loop {
            let interval = {
                let state = app2.state::<SharedState>();
                let st = state.lock().unwrap();
                if !st.ambient.running {
                    return;
                }
                st.ambient.interval_ms
            };
            std::thread::sleep(Duration::from_millis(interval));
            if !sample_if_current(&app2, generation) {
                return;
            }
        }
    });
}

/// 单次采样（对照 sampleAmbientOnce）；返回 false 表示循环应退出
fn sample_if_current(app: &AppHandle, generation: u64) -> bool {
    if GENERATION.load(Ordering::SeqCst) != generation {
        return false;
    }
    let state = app.state::<SharedState>();
    let source = {
        let st = state.lock().unwrap();
        if !st.ambient.running {
            return false;
        }
        st.ambient.source.clone().unwrap_or_else(|| "camera".into())
    };

    // 采样（慢操作 ~0.5–1s，不持锁）
    let (luma, err): (Option<f64>, Option<String>) = if source == "als" {
        let r = als::read_als();
        if let Some(lux) = r.lux {
            (Some(lux), None)
        } else {
            (
                None,
                Some(if r.available { "无读数" } else { "ALS 不可用" }.to_string()),
            )
        }
    } else {
        let r = camera::sample_luma();
        (r.luma, r.error)
    };

    if GENERATION.load(Ordering::SeqCst) != generation {
        return false;
    }

    if let Some(luma) = luma {
        let evt = {
            let mut st = state.lock().unwrap();
            if !st.ambient.running {
                return false;
            }
            st.ambient.failures = 0;
            st.ambient
                .analyzer
                .as_mut()
                .and_then(|a| a.feed(luma, chrono::Local::now().timestamp_millis()))
        };
        if evt == Some(crate::ambient::analyzer::FeedEvent::Darker) {
            handle_ambient_darker(app);
        }
        broadcast_ambient_state(app);
        return true;
    }

    // 失败路径
    let err = err.unwrap_or_default();
    let should_降级 = {
        let mut st = state.lock().unwrap();
        st.ambient.failures += 1;
        eprintln!("[ambient] 采样失败 {}：{}", st.ambient.failures, err);
        if st.ambient.failures >= 2 {
            if st.ambient.source.as_deref() == Some("als") {
                // ALS 连续失败：降级摄像头（而非直接停止监测）
                st.ambient.source = Some("camera".into());
                st.ambient.failures = 0;
                eprintln!("[ambient] ALS 连续失败，已回退摄像头测光");
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if should_降级 {
        broadcast_ambient_state(app);
        return true;
    }
    let should_stop = {
        let st = state.lock().unwrap();
        st.ambient.failures >= 2 && st.ambient.source.as_deref() != Some("als")
    };
    if should_stop {
        stop_ambient_monitor(app, Some(&format!("连续采样失败：{err}")));
        return false;
    }
    broadcast_ambient_state(app);
    true
}

/// 环境变暗处理（对照 handleAmbientDarker）
fn handle_ambient_darker(app: &AppHandle) {
    let state = app.state::<SharedState>();
    let (mode_id, action) = {
        let st = state.lock().unwrap();
        let cfg = st.settings.get().get("ambient").cloned().unwrap_or(json!({}));
        (
            cfg.get("autoModeId")
                .and_then(|v| v.as_str())
                .unwrap_or("night")
                .to_string(),
            cfg.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("notify")
                .to_string(),
        )
    };
    let Some(mode) = crate::modes::get_mode(&mode_id) else {
        return;
    };
    eprintln!("[ambient] 环境变暗 → action={action} mode={mode_id}");

    if action == "auto" {
        crate::commands::apply_mode_with_app(app, &mode_id, true, false);
        // 托盘气泡（notifyBalloon）——W4 托盘接入后补齐
    } else {
        crate::propose::show_propose(
            app,
            json!({
                "kind": "ambient",
                "modeId": mode_id,
                "title": "环境光线变暗了",
                "body": format!("切换到「{} {}K」吗？", mode.name, mode.kelvin)
            }),
        );
    }
}
