//! 配置持久化——对照移植 `src/main/settings.js`
//!
//! - 损坏自愈（解析失败回落默认值，不抛异常）
//! - 深合并（补默认字段、保留用户字段与未知字段——向前兼容）
//! - 原子写（临时文件 + rename）
//! - v1→v2 迁移（preset → modeId）
//!
//! 数据容器采用 serde_json::Value（非强类型 struct）——保真 JS 的「未知字段保留」语义。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// 默认配置（对照 settings.js 的 DEFAULTS）
pub fn defaults() -> Value {
    json!({
        "enabled": true,
        "temperature": 6500,
        "brightness": 100,
        "preset": "off",
        "modeId": "natural",
        "theme": "dark",
        "breaks": { "enabled": true, "workSeconds": 1200, "breakSeconds": 20, "style": "gentle" },
        "hotkeys": { "tempUp": "Control+Alt+Up", "tempDown": "Control+Alt+Down" },
        "autoLaunch": false,
        "pausedUntil": null,
        "schedule": {
            "enabled": false,
            "entries": [
                { "id": "s1", "time": "09:00", "modeId": "office", "action": "auto", "enabled": true },
                { "id": "s2", "time": "18:00", "modeId": "evening", "action": "ask", "enabled": true },
                { "id": "s3", "time": "22:00", "modeId": "night", "action": "auto", "enabled": true }
            ],
            "lastFired": {}
        },
        "ambient": {
            "enabled": false,
            "intervalSeconds": 60,
            "dropThresholdPercent": 35,
            "action": "notify",
            "autoModeId": "night",
            "cooldownMinutes": 15
        },
        "colorSensitive": {
            "enabled": false,
            "apps": ["photoshop", "illustrator", "adobe premiere pro", "afterfx", "resolve", "lightroom", "mspaint", "snippingtool"]
        }
    })
}

/// 深合并：patch 的值与 base 的值同为对象时递归，否则整体替换（对照 deepMerge）
pub fn deep_merge(base: &Value, patch: &Value) -> Value {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            let mut out = b.clone();
            for (k, v) in p {
                let merged = match out.get(k) {
                    Some(existing) => deep_merge(existing, v),
                    None => v.clone(),
                };
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        _ => patch.clone(),
    }
}

/// v1 preset → v2 modeId 映射表（未列出的未知值回落 natural）
const PRESET_TO_MODE_ID: [(&str, &str); 6] = [
    ("off", "natural"),
    ("office", "office"),
    ("evening", "evening"),
    ("night", "night"),
    ("deepnight", "deepnight"),
    ("custom", "custom"),
];

/// v1→v2 迁移：仅当磁盘原始数据没有 modeId 字段、且有 preset 字段时，
/// 按 v1 preset 映射出 modeId 写入合并结果。磁盘已有 modeId 时不动。
fn migrate_preset_to_mode_id(raw: &Value, merged: &mut Value) {
    let Some(raw_obj) = raw.as_object() else { return };
    if raw_obj.contains_key("modeId") {
        return;
    }
    let Some(preset) = raw_obj.get("preset") else { return };
    let mapped = match preset {
        Value::String(s) => PRESET_TO_MODE_ID
            .iter()
            .find(|(k, _)| k == s)
            .map(|(_, v)| *v)
            .unwrap_or("natural"),
        _ => "natural",
    };
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("modeId".to_string(), json!(mapped));
    }
}

/// 配置存储（userData/settings.json）
pub struct SettingsStore {
    file: PathBuf,
    data: Value,
}

impl SettingsStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            file: data_dir.join("settings.json"),
            data: defaults(),
        }
    }

    /// 读取并深合并到默认值（文件缺失/损坏均回落默认值，不抛异常）；含 v1→v2 迁移
    pub fn load(&mut self) -> &Value {
        let disk = fs::read_to_string(&self.file)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| json!({}));
        let mut merged = deep_merge(&defaults(), &disk);
        migrate_preset_to_mode_id(&disk, &mut merged);
        self.data = merged;
        &self.data
    }

    /// 当前内存中的完整配置
    pub fn get(&self) -> &Value {
        &self.data
    }

    /// 合并 patch 并原子落盘（写盘失败返回错误，由调用方处理）
    pub fn save(&mut self, patch: &Value) -> Result<&Value, std::io::Error> {
        self.data = deep_merge(&self.data, patch);
        self.write_atomic()?;
        Ok(&self.data)
    }

    fn write_atomic(&self) -> Result<(), std::io::Error> {
        let mut tmp = self.file.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.data).unwrap_or_else(|_| "{}".to_string());
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.file)?;
        Ok(())
    }
}

// ============================================================
// 测试——对照移植 tests/settings.test.js（18 用例）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eyeguard-settings-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // 'load: 无文件时返回完整默认值'
    #[test]
    fn load_defaults_without_file() {
        let dir = tmp_dir("defaults");
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["temperature"], 6500);
        assert_eq!(data["breaks"]["workSeconds"], 1200);
        assert_eq!(data["brightness"], 100);
    }

    // 'load: 损坏 JSON 自愈为默认值（不抛异常）'
    #[test]
    fn load_heals_broken_json() {
        let dir = tmp_dir("broken");
        fs::write(dir.join("settings.json"), "{broken json!!!").unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["temperature"], 6500);
    }

    // 'load: 部分字段缺失时补默认（深合并）'
    #[test]
    fn load_deep_merges_partial() {
        let dir = tmp_dir("partial");
        fs::write(
            dir.join("settings.json"),
            r#"{"temperature": 4500, "breaks": {"workSeconds": 777}}"#,
        )
        .unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["temperature"], 4500);
        assert_eq!(data["breaks"]["workSeconds"], 777);
        assert_eq!(data["breaks"]["breakSeconds"], 20);
        assert_eq!(data["brightness"], 100);
    }

    // 'load: 未知字段保留（向前兼容）'
    #[test]
    fn load_keeps_unknown_fields() {
        let dir = tmp_dir("unknown");
        fs::write(
            dir.join("settings.json"),
            r#"{"temperature": 4500, "futureField": {"x": 1}}"#,
        )
        .unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["futureField"]["x"], 1);
    }

    // 'save: 合并 patch 并落盘（可读回）'
    #[test]
    fn save_merges_and_persists() {
        let dir = tmp_dir("save");
        let mut store = SettingsStore::new(&dir);
        store.load();
        store.save(&json!({"temperature": 3400})).unwrap();
        store.save(&json!({"breaks": {"enabled": false}})).unwrap();

        let raw: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
        assert_eq!(raw["temperature"], 3400);
        assert_eq!(raw["breaks"]["enabled"], false);
        assert_eq!(raw["breaks"]["workSeconds"], 1200);
    }

    // 'save: 无临时文件残留（原子写清理）'
    #[test]
    fn save_no_tmp_leftover() {
        let dir = tmp_dir("notmp");
        let mut store = SettingsStore::new(&dir);
        store.load();
        store.save(&json!({"temperature": 5000})).unwrap();
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert_eq!(leftovers.len(), 0);
    }

    // 'load: 无文件时 theme/modeId 取 v2 默认值'
    #[test]
    fn load_v2_defaults() {
        let dir = tmp_dir("v2defaults");
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["theme"], "dark");
        assert_eq!(data["modeId"], "natural");
        assert_eq!(defaults()["theme"], "dark");
        assert_eq!(defaults()["modeId"], "natural");
    }

    // 构造一份 v1 配置（无 modeId 字段，含 preset）并返回加载结果
    fn load_v1(tag: &str, preset: &str) -> Value {
        let dir = tmp_dir(tag);
        fs::write(
            dir.join("settings.json"),
            format!(
                r#"{{"temperature": 4500, "preset": "{}", "breaks": {{"workSeconds": 777}}}}"#,
                preset
            ),
        )
        .unwrap();
        let mut store = SettingsStore::new(&dir);
        store.load().clone()
    }

    // '迁移: preset "off" → modeId "natural"'
    #[test]
    fn migrate_off_to_natural() {
        assert_eq!(load_v1("m-off", "off")["modeId"], "natural");
    }

    // '迁移: preset "office" → modeId "office"（同名）'
    #[test]
    fn migrate_office_same_name() {
        let data = load_v1("m-office", "office");
        assert_eq!(data["modeId"], "office");
        assert_eq!(data["preset"], "office", "v1 preset 字段保留不动");
        assert_eq!(data["temperature"], 4500, "其它字段不受迁移影响");
        assert_eq!(data["breaks"]["workSeconds"], 777, "深合并路径不受迁移影响");
    }

    // '迁移: preset "evening" → modeId "evening"（同名）'
    #[test]
    fn migrate_evening_same_name() {
        assert_eq!(load_v1("m-evening", "evening")["modeId"], "evening");
    }

    // '迁移: preset "night" → modeId "night"（同名）'
    #[test]
    fn migrate_night_same_name() {
        assert_eq!(load_v1("m-night", "night")["modeId"], "night");
    }

    // '迁移: preset "deepnight" → modeId "deepnight"（同名）'
    #[test]
    fn migrate_deepnight_same_name() {
        assert_eq!(load_v1("m-deepnight", "deepnight")["modeId"], "deepnight");
    }

    // '迁移: preset "custom" → modeId "custom"'
    #[test]
    fn migrate_custom() {
        assert_eq!(load_v1("m-custom", "custom")["modeId"], "custom");
    }

    // '迁移: 未知 preset 值回落 modeId "natural"'
    #[test]
    fn migrate_unknown_preset_falls_back() {
        assert_eq!(load_v1("m-unknown", "some-future-preset")["modeId"], "natural");
    }

    // '迁移: 无 preset 字段时不迁移（保持默认 natural）'
    #[test]
    fn migrate_skips_without_preset() {
        let dir = tmp_dir("m-nopreset");
        fs::write(dir.join("settings.json"), r#"{"temperature": 4500}"#).unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["modeId"], "natural");
        assert_eq!(data["temperature"], 4500);
    }

    // '迁移: 磁盘已有 modeId 时不被 preset 覆盖'
    #[test]
    fn migrate_explicit_mode_id_wins() {
        let dir = tmp_dir("m-explicit");
        fs::write(
            dir.join("settings.json"),
            r#"{"preset": "night", "modeId": "custom"}"#,
        )
        .unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["modeId"], "custom");
    }

    // '迁移: 损坏 JSON 仍自愈为默认值（迁移不影响自愈路径）'
    #[test]
    fn migrate_does_not_break_healing() {
        let dir = tmp_dir("m-broken");
        fs::write(dir.join("settings.json"), "{broken json!!!").unwrap();
        let mut store = SettingsStore::new(&dir);
        let data = store.load();
        assert_eq!(data["modeId"], "natural");
        assert_eq!(data["theme"], "dark");
    }

    // '迁移: 端到端——老配置加载后 save，迁移值随内存态落盘'
    #[test]
    fn migrate_end_to_end() {
        let dir = tmp_dir("m-e2e");
        fs::write(
            dir.join("settings.json"),
            r#"{"preset": "night", "temperature": 4200, "brightness": 88, "breaks": {"workSeconds": 900}}"#,
        )
        .unwrap();

        let mut store = SettingsStore::new(&dir);
        let loaded = store.load().clone();
        assert_eq!(loaded["modeId"], "night");
        assert_eq!(loaded["theme"], "dark");
        assert_eq!(loaded["temperature"], 4200);
        assert_eq!(loaded["breaks"]["workSeconds"], 900);

        store.save(&json!({"brightness": 70})).unwrap();
        let disk: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
        assert_eq!(disk["modeId"], "night");
        assert_eq!(disk["theme"], "dark");
        assert_eq!(disk["brightness"], 70);
        assert_eq!(disk["temperature"], 4200);
        assert_eq!(disk["preset"], "night");
    }
}
