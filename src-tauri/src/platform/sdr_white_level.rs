//! Windows HDR 下的 SDR 内容亮度（白电平）。
//!
//! 公开 API 只定义了读取结构；Windows 设置页使用的写入类型仍未公开，
//! 因此写入结构与类型值在此集中隔离，失败时按“不支持”处理。

use crate::error::{LumarisError, LumarisResult};
use crate::platform::sdr_values::{percent_to_raw, raw_to_percent};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
    QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_TYPE,
    DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};

const ERROR_SUCCESS: i32 = 0;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const SET_SDR_WHITE_LEVEL: DISPLAYCONFIG_DEVICE_INFO_TYPE = DISPLAYCONFIG_DEVICE_INFO_TYPE(-18);
const GET_ADVANCED_COLOR_INFO_2: DISPLAYCONFIG_DEVICE_INFO_TYPE =
    DISPLAYCONFIG_DEVICE_INFO_TYPE(15);
const ADVANCED_COLOR_MODE_HDR: i32 = 2;

#[repr(C)]
#[derive(Default)]
struct DisplayConfigGetAdvancedColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    flags: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
    active_color_mode: i32,
}

#[repr(C)]
struct DisplayConfigSetSdrWhiteLevel {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    sdr_white_level: u32,
    final_value: u8,
}

#[derive(Clone, Copy)]
struct DisplayTarget {
    adapter_id: windows::Win32::Foundation::LUID,
    target_id: u32,
}

pub fn probe(device_name: &str) -> Option<u32> {
    let target = find_target(device_name).ok()?;
    if !advanced_color_enabled(target) {
        return None;
    }
    read_target(target).ok()
}

pub fn write(device_name: &str, percent: u32, final_write: bool) -> LumarisResult<u32> {
    let target = find_target(device_name)?;
    if !advanced_color_enabled(target) {
        return Err(LumarisError::monitor("当前显示器未启用 HDR"));
    }
    let percent = percent.min(100);
    write_target(target, percent, final_write)?;
    if final_write {
        read_target(target)
    } else {
        Ok(percent)
    }
}

fn advanced_color_enabled(target: DisplayTarget) -> bool {
    let mut info2 = DisplayConfigGetAdvancedColorInfo2::default();
    info2.header.r#type = GET_ADVANCED_COLOR_INFO_2;
    info2.header.size = std::mem::size_of_val(&info2) as u32;
    info2.header.adapterId = target.adapter_id;
    info2.header.id = target.target_id;
    if unsafe { DisplayConfigGetDeviceInfo(&mut info2.header) } == ERROR_SUCCESS {
        const HDR_USER_ENABLED: u32 = 1 << 5;
        return info2.active_color_mode == ADVANCED_COLOR_MODE_HDR
            && (info2.flags & HDR_USER_ENABLED) != 0;
    }

    // Windows 11 24H2 之前没有 activeColorMode，只能回退旧标志。
    let mut info = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO::default();
    info.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
    info.header.size = std::mem::size_of_val(&info) as u32;
    info.header.adapterId = target.adapter_id;
    info.header.id = target.target_id;
    unsafe {
        DisplayConfigGetDeviceInfo(&mut info.header) == ERROR_SUCCESS
            && (info.Anonymous.value & 0b10) != 0
    }
}

fn read_target(target: DisplayTarget) -> LumarisResult<u32> {
    let mut level = DISPLAYCONFIG_SDR_WHITE_LEVEL::default();
    level.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL;
    level.header.size = std::mem::size_of_val(&level) as u32;
    level.header.adapterId = target.adapter_id;
    level.header.id = target.target_id;
    let code = unsafe { DisplayConfigGetDeviceInfo(&mut level.header) };
    if code != ERROR_SUCCESS {
        return Err(LumarisError::win32(
            "DisplayConfigGetDeviceInfo(SDR)",
            code as u32,
        ));
    }
    Ok(raw_to_percent(level.SDRWhiteLevel))
}

fn write_target(target: DisplayTarget, percent: u32, final_write: bool) -> LumarisResult<()> {
    let mut params = DisplayConfigSetSdrWhiteLevel {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER::default(),
        sdr_white_level: percent_to_raw(percent),
        final_value: u8::from(final_write),
    };
    params.header.r#type = SET_SDR_WHITE_LEVEL;
    params.header.size = std::mem::size_of_val(&params) as u32;
    params.header.adapterId = target.adapter_id;
    params.header.id = target.target_id;
    let code = unsafe { DisplayConfigSetDeviceInfo(&params.header) };
    if code != ERROR_SUCCESS {
        return Err(LumarisError::win32(
            "DisplayConfigSetDeviceInfo(SDR)",
            code as u32,
        ));
    }
    Ok(())
}

fn find_target(device_name: &str) -> LumarisResult<DisplayTarget> {
    let mut path_count = 0;
    let mut mode_count = 0;
    let size_code = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if size_code.0 != 0 {
        return Err(LumarisError::win32(
            "GetDisplayConfigBufferSizes",
            size_code.0,
        ));
    }

    for _ in 0..3 {
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let code = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };
        if code.0 == ERROR_INSUFFICIENT_BUFFER {
            let _ = unsafe {
                GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
            };
            continue;
        }
        if code.0 != 0 {
            return Err(LumarisError::win32("QueryDisplayConfig", code.0));
        }
        paths.truncate(path_count as usize);
        for path in paths {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = std::mem::size_of_val(&source) as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != ERROR_SUCCESS {
                continue;
            }
            if wide_to_string(&source.viewGdiDeviceName).eq_ignore_ascii_case(device_name) {
                return Ok(DisplayTarget {
                    adapter_id: path.targetInfo.adapterId,
                    target_id: path.targetInfo.id,
                });
            }
        }
        break;
    }
    Err(LumarisError::monitor("未找到对应的活动显示路径"))
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
