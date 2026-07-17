//! 配置原子读写：临时文件 + rename

use super::migration::migrate_config;
use super::model::{AppConfig, SCHEMA_VERSION};
use crate::error::{LumarisError, LumarisResult};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn app_data_dir() -> LumarisResult<PathBuf> {
    let base = dirs::data_local_dir().ok_or_else(|| {
        LumarisError::config("无法定位 LOCALAPPDATA")
    })?;
    let dir = base.join("Lumaris");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn config_path() -> LumarisResult<PathBuf> {
    Ok(app_data_dir()?.join("config.json"))
}

pub fn logs_dir() -> LumarisResult<PathBuf> {
    let dir = app_data_dir()?.join("logs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn webview2_dir() -> LumarisResult<PathBuf> {
    let dir = app_data_dir()?.join("WebView2");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn load_config() -> LumarisResult<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = AppConfig::default();
        let _ = save_config(&cfg);
        return Ok(cfg);
    }

    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "读取配置失败，使用默认");
            return Ok(AppConfig::default());
        }
    };

    match parse_and_migrate(&raw) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            tracing::error!(error = %e, "配置损坏，备份并恢复默认");
            backup_corrupt(&path);
            let cfg = AppConfig::default();
            let _ = save_config(&cfg);
            Ok(cfg)
        }
    }
}

fn parse_and_migrate(raw: &str) -> LumarisResult<AppConfig> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    let mut cfg = migrate_config(value)?;
    cfg.validate_mut();
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> LumarisResult<()> {
    let path = config_path()?;
    let dir = path
        .parent()
        .ok_or_else(|| LumarisError::config("无效配置路径"))?;
    fs::create_dir_all(dir)?;

    let mut owned = cfg.clone();
    owned.schema_version = SCHEMA_VERSION;
    owned.validate_mut();

    let json = serde_json::to_string_pretty(&owned)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    // 原子替换
    fs::rename(&tmp, &path).or_else(|_| {
        // Windows 上目标存在时 rename 可能失败，先删除再替换
        let _ = fs::remove_file(&path);
        fs::rename(&tmp, &path)
    })?;
    Ok(())
}

fn backup_corrupt(path: &Path) {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup = path.with_extension(format!("json.corrupt.{stamp}"));
    if let Err(e) = fs::copy(path, &backup) {
        tracing::warn!(error = %e, "备份损坏配置失败");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::HotkeyConfig;

    #[test]
    fn default_roundtrip() {
        let cfg = AppConfig::default();
        let s = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert_eq!(back.hotkeys, HotkeyConfig::defaults());
    }
}
