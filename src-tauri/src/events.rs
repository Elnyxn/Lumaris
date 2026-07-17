//! 前后端事件名称与载荷定义

use serde::{Deserialize, Serialize};

pub const EVT_MONITORS_CHANGED: &str = "monitors-changed";
pub const EVT_BRIGHTNESS_CHANGED: &str = "brightness-changed";
pub const EVT_CONTRAST_CHANGED: &str = "contrast-changed";
pub const EVT_OPERATION_RESULT: &str = "operation-result";
pub const EVT_HOTKEY_ACTION: &str = "hotkey-action";
pub const EVT_WINDOW_SHOWN: &str = "window-shown";
pub const EVT_WINDOW_HIDDEN: &str = "window-hidden";
pub const EVT_APP_STATE: &str = "app-state";
pub const EVT_SETTINGS_CHANGED: &str = "settings-changed";
pub const EVT_ERROR: &str = "app-error";
pub const EVT_FRONTEND_READY_ACK: &str = "frontend-ready-ack";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrightnessChangedEvent {
    pub monitor_id: String,
    pub brightness: u32,
    pub cached: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContrastChangedEvent {
    pub monitor_id: String,
    pub contrast: u32,
    pub cached: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResultEvent {
    pub monitor_id: String,
    pub success: bool,
    pub brightness: Option<u32>,
    pub error: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyActionEvent {
    pub action: String,
    pub monitor_id: Option<String>,
    /// 按住连发时中间为 false，松开/单击为 true
    #[serde(default = "default_true")]
    pub final_write: bool,
    /// 步数或百分点（见 by_points）；默认 1
    #[serde(default = "default_steps")]
    pub steps: i32,
    /// true：steps 为绝对百分点；false：steps × 设置步长
    #[serde(default)]
    pub by_points: bool,
}

fn default_true() -> bool {
    true
}

fn default_steps() -> i32 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorEvent {
    pub message: String,
    pub code: String,
}
