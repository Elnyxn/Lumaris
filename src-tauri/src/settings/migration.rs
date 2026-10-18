//! 配置版本迁移

use super::model::{AppConfig, SCHEMA_VERSION};
use crate::error::{LumarisError, LumarisResult};

pub fn migrate_config(mut value: serde_json::Value) -> LumarisResult<AppConfig> {
    let version = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    if version > SCHEMA_VERSION {
        return Err(LumarisError::config(format!(
            "配置版本 {version} 高于当前支持 {SCHEMA_VERSION}"
        )));
    }

    // v0 → v1：补全缺失字段后按默认反序列化
    if version < 1 {
        if let serde_json::Value::Object(ref mut map) = value {
            map.entry("schemaVersion".to_string())
                .or_insert(serde_json::json!(1));
        }
    }

    // 未来版本：在此链式迁移
    // if version < 2 { ... }

    let cfg: AppConfig = serde_json::from_value(value)
        .map_err(|e| LumarisError::config(format!("配置解析失败: {e}")))?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_empty_object() {
        let v = serde_json::json!({});
        let cfg = migrate_config(v).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_v1() {
        let v = serde_json::json!({
            "schemaVersion": 1,
            "stepPercent": 10
        });
        let cfg = migrate_config(v).unwrap();
        assert_eq!(cfg.schema_version, 1);
        assert_eq!(cfg.step_percent, 10);
        assert_eq!(cfg.hotkeys, AppConfig::default().hotkeys);
    }

    #[test]
    fn invalid_field_types_still_fail() {
        assert!(migrate_config(serde_json::json!({"stepPercent": "invalid"})).is_err());
    }

    #[test]
    fn legacy_settings_gain_empty_modes_and_default_height_without_reset() {
        let mut cfg = migrate_config(serde_json::json!({
            "schemaVersion": 1, "stepPercent": 10,
            "cachedBrightness": {"a": 73},
            "brightnessPresets": {"day": {"a": 130, "": 50}, "night": {"a": 25}},
            "flyoutMaxHeightPercent": 150
        }))
        .unwrap();
        cfg.validate_mut();
        assert_eq!(cfg.step_percent, 10);
        assert_eq!(cfg.cached_brightness["a"], 73);
        assert_eq!(cfg.brightness_presets.day["a"], 100);
        assert!(!cfg.brightness_presets.day.contains_key(""));
        assert_eq!(cfg.brightness_presets.night["a"], 25);
        assert_eq!(cfg.flyout_max_height_percent, 100);
        let old =
            migrate_config(serde_json::json!({"schemaVersion": 1, "stepPercent": 10})).unwrap();
        assert_eq!(old.flyout_max_height_percent, 50);
        assert!(old.brightness_presets.day.is_empty());
        assert!(old.brightness_presets.night.is_empty());
        let roundtrip = migrate_config(serde_json::to_value(&cfg).unwrap()).unwrap();
        assert_eq!(roundtrip.brightness_presets, cfg.brightness_presets);
    }
}
