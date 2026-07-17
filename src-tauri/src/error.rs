//! 统一错误类型：业务错误可序列化到前端，技术细节写入日志。

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LumarisError {
    #[error("{0}")]
    Message(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("显示器错误: {0}")]
    Monitor(String),

    #[error("DDC/CI 错误: {0}")]
    Ddc(String),

    #[error("快捷键错误: {0}")]
    Hotkey(String),

    #[error("自启错误: {0}")]
    Autostart(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Win32 {api}: 错误码 {code}")]
    Win32 { api: &'static str, code: u32 },
}

impl LumarisError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn monitor(msg: impl Into<String>) -> Self {
        Self::Monitor(msg.into())
    }

    pub fn ddc(msg: impl Into<String>) -> Self {
        Self::Ddc(msg.into())
    }

    pub fn hotkey(msg: impl Into<String>) -> Self {
        Self::Hotkey(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn win32(api: &'static str, code: u32) -> Self {
        Self::Win32 { api, code }
    }

    /// 给用户看的简短文案
    pub fn user_message(&self) -> String {
        match self {
            Self::Message(m) | Self::Config(m) | Self::Monitor(m) | Self::Ddc(m) | Self::Hotkey(m)
            | Self::Autostart(m) => m.clone(),
            Self::Io(_) => "文件操作失败".into(),
            Self::Serde(_) => "数据格式错误".into(),
            Self::Win32 { .. } => "系统接口调用失败".into(),
        }
    }
}

/// 前端收到的错误载荷
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub message: String,
    pub code: String,
}

impl From<LumarisError> for ErrorPayload {
    fn from(value: LumarisError) -> Self {
        let code = match &value {
            LumarisError::Message(_) => "message",
            LumarisError::Config(_) => "config",
            LumarisError::Monitor(_) => "monitor",
            LumarisError::Ddc(_) => "ddc",
            LumarisError::Hotkey(_) => "hotkey",
            LumarisError::Autostart(_) => "autostart",
            LumarisError::Io(_) => "io",
            LumarisError::Serde(_) => "serde",
            LumarisError::Win32 { .. } => "win32",
        };
        Self {
            message: value.user_message(),
            code: code.into(),
        }
    }
}

pub type LumarisResult<T> = Result<T, LumarisError>;

/// Tauri 命令返回错误字符串（简洁）
impl Serialize for LumarisError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.user_message())
    }
}
