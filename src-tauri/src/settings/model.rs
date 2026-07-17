//! 版本化配置模型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_MONITOR_CONFIG_ENTRIES: usize = 64;
pub const MAX_MONITOR_ID_LEN: usize = 512;
pub const MAX_MONITOR_ALIAS_LEN: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub hotkeys: HotkeyConfig,
    pub step_percent: u32,
    pub custom_step_percent: u32,
    pub autostart: bool,
    pub silent_startup: bool,
    pub delayed_monitor_init: bool,
    pub delayed_init_ms: u64,
    pub osd: OsdSettings,
    pub ui: UiSettings,
    pub target_mode: TargetMode,
    pub last_monitor_id: Option<String>,
    pub fixed_monitor_id: Option<String>,
    pub layout_mode: LayoutMode,
    pub sync_all: bool,
    pub remember_last_monitor: bool,
    pub read_brightness_on_start: bool,
    /// 首屏浮窗是否显示对比度滑块
    #[serde(default)]
    pub show_contrast: bool,
    /// UI 语言：zh-CN | en
    #[serde(default = "default_locale")]
    pub locale: String,
    pub monitor_aliases: HashMap<String, String>,
    pub monitor_sync_include: HashMap<String, bool>,
    pub cached_brightness: HashMap<String, u32>,
    #[serde(default)]
    pub cached_contrast: HashMap<String, u32>,
    pub log_level: LogLevelSetting,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            hotkeys: HotkeyConfig::default(),
            step_percent: 5,
            custom_step_percent: 5,
            autostart: false,
            silent_startup: true,
            delayed_monitor_init: false,
            delayed_init_ms: 2000,
            osd: OsdSettings::default(),
            ui: UiSettings::default(),
            target_mode: TargetMode::MouseMonitor,
            last_monitor_id: None,
            fixed_monitor_id: None,
            layout_mode: LayoutMode::Single,
            sync_all: false,
            remember_last_monitor: true,
            read_brightness_on_start: true,
            show_contrast: false,
            locale: default_locale(),
            monitor_aliases: HashMap::new(),
            monitor_sync_include: HashMap::new(),
            cached_brightness: HashMap::new(),
            cached_contrast: HashMap::new(),
            log_level: LogLevelSetting::Info,
        }
    }
}

fn default_locale() -> String {
    "zh-CN".into()
}

impl AppConfig {
    pub fn effective_step(&self) -> u32 {
        let s = if self.step_percent == 0 {
            self.custom_step_percent.clamp(1, 25)
        } else {
            self.step_percent
        };
        s.clamp(1, 25)
    }

    pub fn validate_mut(&mut self) {
        self.schema_version = SCHEMA_VERSION;
        self.step_percent = match self.step_percent {
            0 | 1 | 2 | 5 | 10 => self.step_percent,
            _ => 5,
        };
        self.custom_step_percent = self.custom_step_percent.clamp(1, 25);
        self.delayed_init_ms = self.delayed_init_ms.clamp(500, 15_000);
        self.osd.auto_hide_ms = self.osd.auto_hide_ms.clamp(500, 10_000);
        self.ui.opacity = self.ui.opacity.clamp(0.5, 1.0);
        // 规范化语言
        let loc = self.locale.trim().to_ascii_lowercase().replace('_', "-");
        self.locale = if loc == "en" || loc.starts_with("en-") {
            "en".into()
        } else {
            "zh-CN".into()
        };
        self.hotkeys.normalize();
        sanitize_monitor_id(&mut self.last_monitor_id);
        sanitize_monitor_id(&mut self.fixed_monitor_id);
        sanitize_aliases(&mut self.monitor_aliases);
        sanitize_percent_cache(&mut self.cached_brightness);
        sanitize_percent_cache(&mut self.cached_contrast);
        sanitize_monitor_map(&mut self.monitor_sync_include);
    }
}

fn sanitize_monitor_id(id: &mut Option<String>) {
    if id
        .as_deref()
        .is_some_and(|value| value.trim().is_empty() || value.chars().count() > MAX_MONITOR_ID_LEN)
    {
        *id = None;
    }
}

fn sanitize_aliases(aliases: &mut HashMap<String, String>) {
    aliases.retain(|id, alias| {
        if id.trim().is_empty() || id.chars().count() > MAX_MONITOR_ID_LEN {
            return false;
        }
        *alias = alias.trim().chars().take(MAX_MONITOR_ALIAS_LEN).collect();
        !alias.is_empty()
    });
    cap_monitor_map(aliases);
}

fn sanitize_percent_cache(cache: &mut HashMap<String, u32>) {
    cache.retain(|id, value| {
        if id.trim().is_empty() || id.chars().count() > MAX_MONITOR_ID_LEN {
            return false;
        }
        *value = (*value).min(100);
        true
    });
    cap_monitor_map(cache);
}

fn sanitize_monitor_map<T>(map: &mut HashMap<String, T>) {
    map.retain(|id, _| !id.trim().is_empty() && id.chars().count() <= MAX_MONITOR_ID_LEN);
    cap_monitor_map(map);
}

fn cap_monitor_map<T>(map: &mut HashMap<String, T>) {
    if map.len() <= MAX_MONITOR_CONFIG_ENTRIES {
        return;
    }
    let mut keys: Vec<_> = map.keys().cloned().collect();
    keys.sort_unstable();
    for key in keys.into_iter().skip(MAX_MONITOR_CONFIG_ENTRIES) {
        map.remove(&key);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConfig {
    pub increase: Option<String>,
    pub decrease: Option<String>,
    pub toggle_flyout: Option<String>,
    pub prev_monitor: Option<String>,
    pub next_monitor: Option<String>,
    pub sync_increase: Option<String>,
    pub sync_decrease: Option<String>,
}

impl HotkeyConfig {
    pub fn defaults() -> Self {
        Self {
            increase: Some("Ctrl+Alt+ArrowUp".into()),
            decrease: Some("Ctrl+Alt+ArrowDown".into()),
            toggle_flyout: Some("Ctrl+Alt+B".into()),
            prev_monitor: Some("Ctrl+Alt+ArrowLeft".into()),
            next_monitor: Some("Ctrl+Alt+ArrowRight".into()),
            sync_increase: None,
            sync_decrease: None,
        }
    }

    pub fn normalize(&mut self) {
        for slot in [
            &mut self.increase,
            &mut self.decrease,
            &mut self.toggle_flyout,
            &mut self.prev_monitor,
            &mut self.next_monitor,
            &mut self.sync_increase,
            &mut self.sync_decrease,
        ] {
            if let Some(s) = slot {
                let t = s.trim().to_string();
                if t.is_empty() {
                    *slot = None;
                } else {
                    *slot = Some(t);
                }
            }
        }
    }

    pub fn iter_actions(&self) -> Vec<(&'static str, Option<&str>)> {
        vec![
            ("increase", self.increase.as_deref()),
            ("decrease", self.decrease.as_deref()),
            ("toggle_flyout", self.toggle_flyout.as_deref()),
            ("prev_monitor", self.prev_monitor.as_deref()),
            ("next_monitor", self.next_monitor.as_deref()),
            ("sync_increase", self.sync_increase.as_deref()),
            ("sync_decrease", self.sync_decrease.as_deref()),
        ]
    }

    pub fn set_action(&mut self, action: &str, value: Option<String>) {
        match action {
            "increase" => self.increase = value,
            "decrease" => self.decrease = value,
            "toggle_flyout" => self.toggle_flyout = value,
            "prev_monitor" => self.prev_monitor = value,
            "next_monitor" => self.next_monitor = value,
            "sync_increase" => self.sync_increase = value,
            "sync_decrease" => self.sync_decrease = value,
            _ => {}
        }
    }

    pub fn get_action(&self, action: &str) -> Option<&str> {
        match action {
            "increase" => self.increase.as_deref(),
            "decrease" => self.decrease.as_deref(),
            "toggle_flyout" => self.toggle_flyout.as_deref(),
            "prev_monitor" => self.prev_monitor.as_deref(),
            "next_monitor" => self.next_monitor.as_deref(),
            "sync_increase" => self.sync_increase.as_deref(),
            "sync_decrease" => self.sync_decrease.as_deref(),
            _ => None,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OsdSettings {
    pub enabled: bool,
    pub auto_hide_ms: u64,
}

impl Default for OsdSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_hide_ms: 1800,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    pub animations: bool,
    pub opacity: f32,
    /// 界面主题：dark=当前深色，light=浅色
    #[serde(default)]
    pub theme: ThemeMode,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            animations: true,
            opacity: 1.0,
            theme: ThemeMode::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum TargetMode {
    #[default]
    MouseMonitor,
    LastUsed,
    Primary,
    Fixed,
    AllSync,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LayoutMode {
    #[default]
    Single,
    List,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LogLevelSetting {
    #[default]
    Info,
    Debug,
    Warn,
    Error,
}

impl LogLevelSetting {
    pub fn as_filter(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}
