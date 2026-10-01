//! 场景模式表（纯数据模块）——对照移植 `src/main/modes.js`
//!
//! 每档 = { 色温 kelvin(K), 亮度 brightness(%) } 组合；数组顺序即 UI 网格呈现顺序。
//! 数值依据：docs/eye-parameters-research.md（f.lux 实践值、办公照明 CCT、昼夜景节律）。

/// 模式档
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mode {
    pub id: &'static str,
    pub name: &'static str,
    pub kelvin: u32,
    pub brightness: u32,
}

/// 7 档场景模式（顺序即 UI 网格呈现顺序）
pub const MODES: [Mode; 7] = [
    Mode { id: "natural", name: "原色", kelvin: 6500, brightness: 100 },
    Mode { id: "focus", name: "冷白提神", kelvin: 8000, brightness: 100 },
    Mode { id: "office", name: "办公", kelvin: 5500, brightness: 100 },
    Mode { id: "reading", name: "阅读暖白", kelvin: 5000, brightness: 90 },
    Mode { id: "evening", name: "傍晚", kelvin: 4500, brightness: 80 },
    Mode { id: "night", name: "夜晚", kelvin: 3400, brightness: 70 },
    Mode { id: "deepnight", name: "深夜", kelvin: 2700, brightness: 60 },
];

/// 默认模式：不干预的原色档
pub const DEFAULT_MODE_ID: &str = "natural";

/// 按 id 查模式（大小写敏感；未命中返回 None）
pub fn get_mode(id: &str) -> Option<&'static Mode> {
    MODES.iter().find(|m| m.id == id)
}

// ============================================================
// 测试——对照移植 tests/modes.test.js（8 用例）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    // 'MODES: id 全局唯一'
    #[test]
    fn mode_ids_unique() {
        let mut ids: Vec<&str> = MODES.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "存在重复 id");
    }

    // 'MODES: 每档字段完整且类型正确'（Rust 类型系统保证整数性，断言非空与范围）
    #[test]
    fn mode_fields_valid() {
        for m in MODES.iter() {
            assert!(!m.id.is_empty(), "id 不得为空串");
            assert!(!m.name.is_empty(), "{} 的 name 不得为空串", m.id);
        }
    }

    // 'MODES: 每档 kelvin ∈ [2000, 10000]'
    #[test]
    fn mode_kelvin_in_range() {
        for m in MODES.iter() {
            assert!(
                (2000..=10000).contains(&m.kelvin),
                "{} kelvin={} 越界 [2000, 10000]",
                m.id,
                m.kelvin
            );
        }
    }

    // 'MODES: 每档 brightness ∈ [50, 100]'
    #[test]
    fn mode_brightness_in_range() {
        for m in MODES.iter() {
            assert!(
                (50..=100).contains(&m.brightness),
                "{} brightness={} 越界 [50, 100]",
                m.id,
                m.brightness
            );
        }
    }

    // 'MODES: 与 §4.2 表格逐档一致（id/kelvin/brightness）'
    #[test]
    fn modes_match_design_table() {
        let expected: [(&str, u32, u32); 7] = [
            ("natural", 6500, 100),
            ("focus", 8000, 100),
            ("office", 5500, 100),
            ("reading", 5000, 90),
            ("evening", 4500, 80),
            ("night", 3400, 70),
            ("deepnight", 2700, 60),
        ];
        assert_eq!(MODES.len(), expected.len());
        for (m, (id, kelvin, brightness)) in MODES.iter().zip(expected.iter()) {
            assert_eq!(m.id, *id);
            assert_eq!(m.kelvin, *kelvin);
            assert_eq!(m.brightness, *brightness);
        }
    }

    // 'getMode: 每一档都能按 id 命中且返回同值对象'
    #[test]
    fn get_mode_hits_every_entry() {
        for m in MODES.iter() {
            let got = get_mode(m.id).expect("应命中");
            assert_eq!(got.id, m.id);
            assert_eq!(got.kelvin, m.kelvin);
            assert_eq!(got.brightness, m.brightness);
            assert_eq!(got.name, m.name);
        }
    }

    // 'getMode: 未命中返回 null（未知 id / 空串 / 自定义态）'
    #[test]
    fn get_mode_misses() {
        assert!(get_mode("no-such-mode").is_none());
        assert!(get_mode("").is_none());
        assert!(get_mode("custom").is_none());
        assert!(get_mode("NATURAL").is_none()); // 大小写敏感
    }

    // 'DEFAULT_MODE_ID: 存在于模式表内，且为原色档'
    #[test]
    fn default_mode_is_natural() {
        let def = get_mode(DEFAULT_MODE_ID).expect("默认档必须可查");
        assert_eq!(DEFAULT_MODE_ID, "natural");
        assert_eq!(def.kelvin, 6500);
        assert_eq!(def.brightness, 100);
    }
}
