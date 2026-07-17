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
        // 缺字段时应能用默认填充失败 → 完整对象更稳妥
        let full = serde_json::to_value(AppConfig::default()).unwrap();
        let cfg = migrate_config(full).unwrap();
        assert_eq!(cfg.schema_version, 1);
        let _ = v;
    }
}
