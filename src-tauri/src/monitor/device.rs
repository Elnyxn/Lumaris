//! 显示器设备模型（前端可见字段 + 内部状态）

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ControlMethod {
    StandardBrightnessApi,
    VcpCode10,
    /// 笔记本内屏 ACPI/WMI 背光（非 DDC/CI）
    WmiAcpi,
    Unsupported,
}

impl ControlMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StandardBrightnessApi => "StandardBrightnessApi",
            Self::VcpCode10 => "VcpCode10",
            Self::WmiAcpi => "WmiAcpi",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MonitorStatus {
    Available,
    Cached,
    TemporarilyOffline,
    Unsupported,
    ReadFailed,
    WriteFailed,
    Sleeping,
    Disconnected,
}

impl MonitorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Cached => "Cached",
            Self::TemporarilyOffline => "TemporarilyOffline",
            Self::Unsupported => "Unsupported",
            Self::ReadFailed => "ReadFailed",
            Self::WriteFailed => "WriteFailed",
            Self::Sleeping => "Sleeping",
            Self::Disconnected => "Disconnected",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorkArea {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// 发送给前端的显示器信息（无句柄）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: String,
    pub display_name: String,
    pub user_alias: Option<String>,
    pub description: String,
    pub current_brightness: u32,
    pub cached_brightness: u32,
    pub min_brightness: u32,
    pub max_brightness: u32,
    pub current_contrast: u32,
    pub cached_contrast: u32,
    pub min_contrast: u32,
    pub max_contrast: u32,
    /// 是否支持 DDC 对比度（VCP 0x12）
    pub contrast_controllable: bool,
    pub control_method: ControlMethod,
    pub status: MonitorStatus,
    pub is_primary: bool,
    pub is_external: bool,
    pub is_selected: bool,
    pub is_controllable: bool,
    pub work_area: WorkArea,
    pub dpi: u32,
}

impl MonitorInfo {
    pub fn label(&self) -> String {
        self.user_alias
            .clone()
            .unwrap_or_else(|| self.display_name.clone())
    }
}

/// 内部设备运行时状态（含句柄由平台层持有）
#[derive(Debug, Clone)]
pub struct DeviceRuntime {
    pub info: MonitorInfo,
    pub cool_until: Option<std::time::Instant>,
    pub last_error: Option<String>,
}
