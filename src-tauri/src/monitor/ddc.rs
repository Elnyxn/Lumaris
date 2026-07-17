//! DDC/CI 操作封装：委托平台层

use super::device::ControlMethod;
use crate::error::LumarisResult;
use crate::platform;

pub fn read_brightness(id: &str) -> LumarisResult<(u32, u32, u32, ControlMethod)> {
    platform::read_brightness(id)
}

pub fn write_brightness(id: &str, percent: u32) -> LumarisResult<(u32, ControlMethod)> {
    platform::write_brightness(id, percent.min(100))
}

pub fn read_contrast(id: &str) -> LumarisResult<(u32, u32, u32)> {
    platform::read_contrast(id)
}

pub fn write_contrast(id: &str, percent: u32) -> LumarisResult<u32> {
    platform::write_contrast(id, percent.min(100))
}

pub fn enumerate() -> LumarisResult<Vec<super::device::MonitorInfo>> {
    platform::enumerate_monitors()
}
