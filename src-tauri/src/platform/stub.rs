//! 非 Windows 桩实现：仅供交叉开发时通过类型检查，不提供硬件控制。

use crate::error::{LumarisError, LumarisResult};
use crate::monitor::device::{ControlMethod, MonitorInfo, MonitorStatus};

pub fn is_windows11_or_greater() -> bool {
    false
}

pub fn primary_work_area() -> crate::monitor::device::WorkArea {
    crate::monitor::device::WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    }
}

pub fn primary_dpi() -> u32 {
    96
}

pub fn apply_acrylic_backdrop(_hwnd: isize) -> LumarisResult<()> {
    Err(LumarisError::message("仅支持 Windows"))
}

pub fn get_cursor_monitor_id(_monitors: &[MonitorInfo]) -> Option<String> {
    None
}

pub fn enumerate_monitors() -> LumarisResult<Vec<MonitorInfo>> {
    Ok(vec![MonitorInfo {
        id: "stub-primary".into(),
        display_name: "Stub Display".into(),
        user_alias: None,
        description: "Non-Windows stub".into(),
        current_brightness: 50,
        cached_brightness: 50,
        min_brightness: 0,
        max_brightness: 100,
        current_contrast: 50,
        cached_contrast: 50,
        min_contrast: 0,
        max_contrast: 100,
        contrast_controllable: false,
        control_method: ControlMethod::Unsupported,
        status: MonitorStatus::Unsupported,
        is_primary: true,
        is_external: false,
        is_selected: true,
        is_controllable: false,
        work_area: Default::default(),
        dpi: 96,
    }])
}

pub fn read_brightness(_id: &str) -> LumarisResult<(u32, u32, u32, ControlMethod)> {
    Err(LumarisError::ddc("非 Windows 平台"))
}

pub fn write_brightness(
    _id: &str,
    _percent: u32,
) -> LumarisResult<(u32, ControlMethod)> {
    Err(LumarisError::ddc("非 Windows 平台"))
}

pub fn read_contrast(_id: &str) -> LumarisResult<(u32, u32, u32)> {
    Err(LumarisError::ddc("非 Windows 平台"))
}

pub fn write_contrast(_id: &str, _percent: u32) -> LumarisResult<u32> {
    Err(LumarisError::ddc("非 Windows 平台"))
}

pub fn install_system_message_hook(_hwnd: isize) -> LumarisResult<()> {
    Ok(())
}

pub fn open_path_in_explorer(path: &str) -> LumarisResult<()> {
    tracing::info!(path, "stub open path");
    Ok(())
}
