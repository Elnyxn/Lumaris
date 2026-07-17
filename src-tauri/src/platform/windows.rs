//! Windows 平台：枚举显示器、DDC/CI、Acrylic 背景、系统消息

use crate::error::{LumarisError, LumarisResult};
use crate::monitor::device::{ControlMethod, MonitorInfo, MonitorStatus, WorkArea};
use crate::monitor::identity::stable_monitor_id;
use crate::utils::{percent_to_raw, raw_to_percent};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, DISPLAY_DEVICEW,
    DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_PRIMARY_DEVICE,
};

/// MONITORINFOF_PRIMARY
const MONITORINFOF_PRIMARY: u32 = 0x0000_0001;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetMonitorCapabilities,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR,
    GetVCPFeatureAndVCPFeatureReply, SetMonitorBrightness, SetVCPFeature, PHYSICAL_MONITOR,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetCursorPos, GetWindowLongPtrW, SetWindowLongPtrW,
    SetWindowPos, GWLP_WNDPROC, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, WM_DEVICECHANGE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_POWERBROADCAST, WM_SETTINGCHANGE, WM_THEMECHANGED, WNDPROC,
};

const VCP_BRIGHTNESS: u8 = 0x10;
const VCP_CONTRAST: u8 = 0x12;
const MC_CAPS_BRIGHTNESS: u32 = 0x2;

/// 物理句柄缓存：id → (HMONITOR raw, physical handle raw, description)
struct PhysicalEntry {
    hmonitor: isize,
    physical: PHYSICAL_MONITOR,
}

// PHYSICAL_MONITOR 含 HANDLE，需手动销毁
unsafe impl Send for PhysicalEntry {}
unsafe impl Sync for PhysicalEntry {}

static PHYSICAL_CACHE: Lazy<RwLock<HashMap<String, PhysicalEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// 走 WMI/ACPI 背光的显示器 id（通常是笔记本主屏）
static WMI_BACKLIGHT_IDS: Lazy<RwLock<std::collections::HashSet<String>>> =
    Lazy::new(|| RwLock::new(std::collections::HashSet::new()));

static ENUM_ACCUM: Lazy<RwLock<Vec<RawEnum>>> = Lazy::new(|| RwLock::new(Vec::new()));

struct RawEnum {
    hmonitor: HMONITOR,
    info: MONITORINFOEXW,
    dpi: u32,
}

unsafe impl Send for RawEnum {}
unsafe impl Sync for RawEnum {}

pub fn is_windows11_or_greater() -> bool {
    // 简单探测：build >= 22000
    unsafe {
        let mut vi = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        if GetVersionExW(&mut vi).is_ok() {
            return vi.dwMajorVersion > 10
                || (vi.dwMajorVersion == 10 && vi.dwBuildNumber >= 22000);
        }
    }
    // 回退：尝试 DWMWA_SYSTEMBACKDROP_TYPE
    true
}

pub fn apply_acrylic_backdrop(hwnd_raw: isize) -> LumarisResult<()> {
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        return Err(LumarisError::message("无效 HWND"));
    }

    unsafe {
        // 系统圆角（唯一圆角来源，CSS 不再二次圆角）
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );

        // 深色
        let dark: BOOL = BOOL(1);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const _,
            std::mem::size_of_val(&dark) as u32,
        );

        // 彻底去掉系统窗口描边（白边/网页框主因之一）
        // DWMWA_COLOR_NONE = 0xFFFFFFFE
        let color_none: u32 = 0xFFFF_FFFE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color_none as *const _ as *const _,
            std::mem::size_of_val(&color_none) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &color_none as *const _ as *const _,
            std::mem::size_of_val(&color_none) as u32,
        );

        if is_windows11_or_greater() {
            let backdrop: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
            let hr = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop as *const _ as *const _,
                std::mem::size_of_val(&backdrop) as u32,
            );
            if hr.is_ok() {
                tracing::info!("已应用 DWMSBT_TRANSIENTWINDOW Acrylic");
                return Ok(());
            }
            tracing::warn!("DWMWA_SYSTEMBACKDROP_TYPE 失败，使用 CSS 材质");
        }

        // 注意：Win10 SetWindowCompositionAttribute 与 WebView2 透明窗口叠加时
        // 容易导致点击穿透/无法拖动，故不再使用；由前端 CSS 半透明材质兜底。
        tracing::info!("使用 CSS 半透明材质（避免 Composition 点击穿透）");
    }
    Ok(())
}

/// Windows 10：动态加载 SetWindowCompositionAttribute（保留备用，默认不用）
#[allow(dead_code)]
unsafe fn apply_win10_acrylic(hwnd: HWND) -> LumarisResult<()> {
    #[repr(C)]
    struct AccentPolicy {
        accent_state: u32,
        accent_flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }
    #[repr(C)]
    struct WindowCompositionAttributeData {
        attribute: u32,
        data: *mut std::ffi::c_void,
        size: usize,
    }

    type SetWCA = unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> i32;

    let module = LoadLibraryW(PCWSTR(
        windows::core::w!("user32.dll").as_ptr(),
    ))
    .map_err(|e| LumarisError::message(format!("LoadLibrary user32 失败: {e}")))?;

    let proc = GetProcAddress(module, windows::core::s!("SetWindowCompositionAttribute"));
    let Some(proc) = proc else {
        return Err(LumarisError::message(
            "SetWindowCompositionAttribute 不可用，将使用 CSS 降级",
        ));
    };
    let set_wca: SetWCA = std::mem::transmute(proc);

    // ACCENT_ENABLE_ACRYLICBLURBEHIND = 4；颜色 ABGR，A=约 0xCC，暖灰褐
    let mut policy = AccentPolicy {
        accent_state: 4,
        accent_flags: 2,
        gradient_color: 0xCC_3A_36_32, // A B G R
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: &mut policy as *mut _ as *mut _,
        size: std::mem::size_of::<AccentPolicy>(),
    };
    let ok = set_wca(hwnd, &mut data);
    if ok == 0 {
        return Err(LumarisError::win32("SetWindowCompositionAttribute", 0));
    }
    tracing::info!("已应用 Win10 Acrylic Blur");
    Ok(())
}

/// 主显示器 HMONITOR（真正的 Primary，不是光标所在屏）
fn primary_hmonitor() -> HMONITOR {
    unsafe {
        // (0,0) + DEFAULTTOPRIMARY → 主显示器
        let pt = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY)
    }
}

/// 主显示器工作区（物理像素）。浮窗固定锚在主屏，不跟副屏/光标跑。
pub fn primary_work_area() -> WorkArea {
    unsafe {
        let hm = primary_hmonitor();
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        if GetMonitorInfoW(hm, &mut info as *mut _ as *mut _).as_bool() {
            // 校验 PRIMARY 标志；若失败再扫一遍
            let r = info.monitorInfo.rcWork;
            if (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0
                || (r.left != 0 || r.top != 0 || r.right != 0)
            {
                return WorkArea {
                    left: r.left,
                    top: r.top,
                    right: r.right,
                    bottom: r.bottom,
                };
            }
        }
    }
    WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    }
}

pub fn primary_dpi() -> u32 {
    unsafe {
        let hm = primary_hmonitor();
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(hm, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        if dpi_x == 0 {
            96
        } else {
            dpi_x
        }
    }
}

pub fn get_cursor_monitor_id(monitors: &[MonitorInfo]) -> Option<String> {
    unsafe {
        let mut pt = windows::Win32::Foundation::POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }
        let hm = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let raw = hm.0 as isize;
        monitors
            .iter()
            .find(|m| {
                PHYSICAL_CACHE
                    .read()
                    .get(&m.id)
                    .map(|e| e.hmonitor == raw)
                    .unwrap_or(false)
            })
            .map(|m| m.id.clone())
    }
}

unsafe extern "system" fn enum_mon_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    _lparam: LPARAM,
) -> BOOL {
    let mut info = MONITORINFOEXW {
        monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _).as_bool() {
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        ENUM_ACCUM.write().push(RawEnum {
            hmonitor,
            info,
            dpi: dpi_x,
        });
    }
    BOOL(1)
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn get_display_device(device_name: &str) -> (String, String, String, bool, bool) {
    // returns: device_string, device_id, device_key-ish, is_primary, is_active
    unsafe {
        let mut i = 0u32;
        loop {
            let mut dd = DISPLAY_DEVICEW {
                cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            let ok = EnumDisplayDevicesW(PCWSTR::null(), i, &mut dd, 0);
            if !ok.as_bool() {
                break;
            }
            let name = wide_to_string(&dd.DeviceName);
            if name.eq_ignore_ascii_case(device_name) {
                let is_primary =
                    (dd.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) == DISPLAY_DEVICE_PRIMARY_DEVICE;
                let is_active = (dd.StateFlags & DISPLAY_DEVICE_ACTIVE) == DISPLAY_DEVICE_ACTIVE
                    || (dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP)
                        == DISPLAY_DEVICE_ATTACHED_TO_DESKTOP;
                // 再枚举监视器设备
                let mut mon = DISPLAY_DEVICEW {
                    cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
                    ..Default::default()
                };
                let mut device_string = wide_to_string(&dd.DeviceString);
                let mut device_id = wide_to_string(&dd.DeviceID);
                if EnumDisplayDevicesW(
                    PCWSTR(windows::core::HSTRING::from(device_name).as_ptr()),
                    0,
                    &mut mon,
                    0,
                )
                .as_bool()
                {
                    let s = wide_to_string(&mon.DeviceString);
                    if !s.is_empty() {
                        device_string = s;
                    }
                    let id = wide_to_string(&mon.DeviceID);
                    if !id.is_empty() {
                        device_id = id;
                    }
                }
                return (device_string, device_id, name, is_primary, is_active);
            }
            i += 1;
        }
    }
    (device_name.to_string(), String::new(), device_name.to_string(), false, true)
}

pub fn clear_physical_cache() {
    let mut map = PHYSICAL_CACHE.write();
    for (_, entry) in map.drain() {
        unsafe {
            let mut arr = [entry.physical];
            let _ = DestroyPhysicalMonitors(&mut arr);
        }
    }
}

pub fn enumerate_monitors() -> LumarisResult<Vec<MonitorInfo>> {
    // 先清空旧句柄
    clear_physical_cache();
    ENUM_ACCUM.write().clear();

    unsafe {
        let ok = EnumDisplayMonitors(None, None, Some(enum_mon_proc), LPARAM(0));
        if !ok.as_bool() {
            return Err(LumarisError::win32("EnumDisplayMonitors", 0));
        }
    }

    let raws = std::mem::take(&mut *ENUM_ACCUM.write());
    let mut result = Vec::new();
    let mut cache = PHYSICAL_CACHE.write();
    // 枚举阶段旁路：monitor id → DeviceID（用于 WMI InstanceName 匹配）
    let mut device_paths: HashMap<String, String> = HashMap::new();

    for raw in raws {
        let device_name = wide_to_string(&raw.info.szDevice);
        let (device_string, device_id, _, is_primary, _active) = get_display_device(&device_name);
        let work = raw.info.monitorInfo.rcWork;
        let work_area = WorkArea {
            left: work.left,
            top: work.top,
            right: work.right,
            bottom: work.bottom,
        };

        let physicals = unsafe { get_physical_monitors(raw.hmonitor) };
        if physicals.is_empty() {
            // 无物理监视器时仍列出逻辑显示器（通常内屏可能无 DDC）
            let desc = if device_string.is_empty() {
                device_name.clone()
            } else {
                device_string.clone()
            };
            let id = stable_monitor_id(&[
                &device_name,
                &device_id,
                &device_string,
                "logical-only",
            ]);
            device_paths.insert(id.clone(), device_id.clone());
            let is_external = !is_primary;
            result.push(MonitorInfo {
                id: id.clone(),
                display_name: desc.clone(),
                user_alias: None,
                description: format!("{desc} ({device_name})"),
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
                is_primary,
                is_external,
                is_selected: false,
                is_controllable: false,
                work_area,
                dpi: raw.dpi,
            });
            continue;
        }

        for (idx, phys) in physicals.into_iter().enumerate() {
            // PHYSICAL_MONITOR 为 packed，不可直接取字段引用
            let desc_buf = phys.szPhysicalMonitorDescription;
            let desc = wide_to_string(&desc_buf);
            let id = stable_monitor_id(&[
                &device_name,
                &device_id,
                &device_string,
                &desc,
                &idx.to_string(),
            ]);

            cache.insert(
                id.clone(),
                PhysicalEntry {
                    hmonitor: raw.hmonitor.0 as isize,
                    physical: phys,
                },
            );
            device_paths.insert(id.clone(), device_id.clone());

            let display_name = if !device_string.is_empty() {
                // 优先友好名（Lenovo / HKC），物理描述常是 Generic PnP
                device_string.clone()
            } else if !desc.is_empty() {
                desc.clone()
            } else {
                device_name.clone()
            };

            // 探测能力（不在 UI 线程长时间阻塞：枚举阶段允许一次）
            let (method, min_b, max_b, cur, status, controllable) =
                unsafe { probe_brightness(&phys) };
            let (c_ok, c_min, c_max, c_cur) = unsafe { probe_contrast(&phys) };

            result.push(MonitorInfo {
                id,
                display_name,
                user_alias: None,
                description: format!("{desc} | {device_name} | {device_id}"),
                current_brightness: cur,
                cached_brightness: cur,
                min_brightness: min_b,
                max_brightness: max_b,
                current_contrast: c_cur,
                cached_contrast: c_cur,
                min_contrast: c_min,
                max_contrast: c_max,
                contrast_controllable: c_ok,
                control_method: method,
                status,
                is_primary,
                is_external: !is_primary || controllable,
                is_selected: false,
                is_controllable: controllable,
                work_area: work_area.clone(),
                dpi: raw.dpi,
            });
        }
    }

    drop(cache);

    // 笔记本内屏 WMI：必须用 InstanceName 硬件 ID 匹配，禁止「仅第二屏幕」时绑到外接屏
    WMI_BACKLIGHT_IDS.write().clear();
    if let Some(probe) = crate::platform::wmi_backlight::probe_wmi() {
        let mut bound = false;
        for m in result.iter_mut() {
            // 已能 DDC 控制的绝不覆盖
            if m.is_controllable {
                continue;
            }
            let dev_id = device_paths.get(&m.id).map(|s| s.as_str()).unwrap_or("");
            if !crate::platform::wmi_backlight::device_matches_wmi(
                dev_id,
                &m.description,
                &m.display_name,
                &probe,
            ) {
                continue;
            }
            let prev = m.control_method.as_str().to_string();
            m.control_method = ControlMethod::WmiAcpi;
            m.is_controllable = true;
            m.is_external = false;
            m.status = MonitorStatus::Available;
            m.current_brightness = probe.percent;
            m.cached_brightness = probe.percent;
            m.min_brightness = 0;
            m.max_brightness = 100;
            if !m.contrast_controllable {
                m.current_contrast = 50;
                m.cached_contrast = 50;
            }
            WMI_BACKLIGHT_IDS.write().insert(m.id.clone());
            bound = true;
            tracing::info!(
                id = %crate::logging::short_id(&m.id),
                percent = probe.percent,
                prev_method = %prev,
                device_id = %dev_id,
                keys = ?probe.hardware_keys,
                "已绑定笔记本 WMI/ACPI 背光（硬件 ID 匹配）"
            );
            break;
        }
        if !bound {
            tracing::info!(
                keys = ?probe.hardware_keys,
                monitors = result.len(),
                "WMI 背光存在但当前枚举屏无匹配（多为「仅第二屏幕」），不绑定"
            );
        }
    }

    tracing::info!(count = result.len(), "显示器枚举完成");
    Ok(result)
}

fn is_wmi_id(id: &str) -> bool {
    WMI_BACKLIGHT_IDS.read().contains(id)
}

unsafe fn get_physical_monitors(hmonitor: HMONITOR) -> Vec<PHYSICAL_MONITOR> {
    let mut count = 0u32;
    if GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count).is_err() || count == 0 {
        return Vec::new();
    }
    let mut buf = vec![PHYSICAL_MONITOR::default(); count as usize];
    if GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut buf).is_err() {
        return Vec::new();
    }
    buf
}

unsafe fn probe_brightness(
    phys: &PHYSICAL_MONITOR,
) -> (ControlMethod, u32, u32, u32, MonitorStatus, bool) {
    // 1) 标准亮度 API（windows-rs 0.61：这些 API 返回 i32/BOOL）
    let mut caps = 0u32;
    let mut color_temp = 0u32;
    if GetMonitorCapabilities(phys.hPhysicalMonitor, &mut caps, &mut color_temp) != 0
        && (caps & MC_CAPS_BRIGHTNESS) != 0
    {
        let mut min_b = 0u32;
        let mut cur = 0u32;
        let mut max_b = 0u32;
        if GetMonitorBrightness(phys.hPhysicalMonitor, &mut min_b, &mut cur, &mut max_b) != 0 {
            let pct = raw_to_percent(cur, min_b, max_b);
            return (
                ControlMethod::StandardBrightnessApi,
                min_b,
                max_b,
                pct,
                MonitorStatus::Available,
                true,
            );
        }
    }

    // 2) VCP 0x10
    let mut current = 0u32;
    let mut maximum = 0u32;
    if GetVCPFeatureAndVCPFeatureReply(
        phys.hPhysicalMonitor,
        VCP_BRIGHTNESS,
        None,
        &mut current,
        Some(&mut maximum as *mut u32),
    ) != 0
        && maximum > 0
    {
        let pct = raw_to_percent(current, 0, maximum);
        return (
            ControlMethod::VcpCode10,
            0,
            maximum,
            pct,
            MonitorStatus::Available,
            true,
        );
    }

    (
        ControlMethod::Unsupported,
        0,
        100,
        50,
        MonitorStatus::Unsupported,
        false,
    )
}

pub fn read_brightness(id: &str) -> LumarisResult<(u32, u32, u32, ControlMethod)> {
    // 仅对枚举阶段已匹配的内屏走 WMI；禁止 DDC 失败后误绑外接屏
    if is_wmi_id(id) {
        let pct = crate::platform::wmi_backlight::read_brightness()?;
        return Ok((pct, 0, 100, ControlMethod::WmiAcpi));
    }

    let cache = PHYSICAL_CACHE.read();
    let entry = cache
        .get(id)
        .ok_or_else(|| LumarisError::monitor("显示器不存在或已断开"))?;
    let phys = entry.physical;
    drop(cache);

    unsafe {
        let mut min_b = 0u32;
        let mut cur = 0u32;
        let mut max_b = 0u32;
        if GetMonitorBrightness(phys.hPhysicalMonitor, &mut min_b, &mut cur, &mut max_b) != 0
            && max_b >= min_b
        {
            let pct = raw_to_percent(cur, min_b, max_b);
            return Ok((pct, min_b, max_b, ControlMethod::StandardBrightnessApi));
        }

        let mut current = 0u32;
        let mut maximum = 0u32;
        if GetVCPFeatureAndVCPFeatureReply(
            phys.hPhysicalMonitor,
            VCP_BRIGHTNESS,
            None,
            &mut current,
            Some(&mut maximum as *mut u32),
        ) != 0
            && maximum > 0
        {
            let pct = raw_to_percent(current, 0, maximum);
            return Ok((pct, 0, maximum, ControlMethod::VcpCode10));
        }
    }

    Err(LumarisError::ddc("读取亮度失败"))
}

pub fn write_brightness(id: &str, percent: u32) -> LumarisResult<(u32, ControlMethod)> {
    let percent = percent.min(100);

    if is_wmi_id(id) {
        crate::platform::wmi_backlight::write_brightness(percent)?;
        return Ok((percent, ControlMethod::WmiAcpi));
    }

    let cache = PHYSICAL_CACHE.read();
    let entry = cache
        .get(id)
        .ok_or_else(|| LumarisError::monitor("显示器不存在或已断开"))?;
    let phys = entry.physical;
    drop(cache);

    // 先探测范围再写入
    unsafe {
        let mut min_b = 0u32;
        let mut cur = 0u32;
        let mut max_b = 0u32;
        if GetMonitorBrightness(phys.hPhysicalMonitor, &mut min_b, &mut cur, &mut max_b) != 0
            && max_b > min_b
        {
            let raw = percent_to_raw(percent, min_b, max_b);
            if SetMonitorBrightness(phys.hPhysicalMonitor, raw) != 0 {
                return Ok((percent, ControlMethod::StandardBrightnessApi));
            }
            tracing::warn!(
                id = %crate::logging::short_id(id),
                "SetMonitorBrightness 失败，尝试 VCP"
            );
        }

        let mut current = 0u32;
        let mut maximum = 0u32;
        if GetVCPFeatureAndVCPFeatureReply(
            phys.hPhysicalMonitor,
            VCP_BRIGHTNESS,
            None,
            &mut current,
            Some(&mut maximum as *mut u32),
        ) != 0
            && maximum > 0
        {
            let raw = percent_to_raw(percent, 0, maximum);
            if SetVCPFeature(phys.hPhysicalMonitor, VCP_BRIGHTNESS, raw) != 0 {
                return Ok((percent, ControlMethod::VcpCode10));
            }
        }
    }

    // 禁止：DDC 失败后把任意屏标成 WMI（「仅第二屏幕」时 WMI 仍是内屏通道）
    Err(LumarisError::ddc("写入亮度失败"))
}

/// 探测对比度（VCP 0x12）
unsafe fn probe_contrast(phys: &PHYSICAL_MONITOR) -> (bool, u32, u32, u32) {
    let mut current = 0u32;
    let mut maximum = 0u32;
    if GetVCPFeatureAndVCPFeatureReply(
        phys.hPhysicalMonitor,
        VCP_CONTRAST,
        None,
        &mut current,
        Some(&mut maximum as *mut u32),
    ) != 0
        && maximum > 0
    {
        let pct = raw_to_percent(current, 0, maximum);
        return (true, 0, maximum, pct);
    }
    (false, 0, 100, 50)
}

pub fn read_contrast(id: &str) -> LumarisResult<(u32, u32, u32)> {
    let cache = PHYSICAL_CACHE.read();
    let entry = cache
        .get(id)
        .ok_or_else(|| LumarisError::monitor("显示器不存在或已断开"))?;
    let phys = entry.physical;
    drop(cache);

    unsafe {
        let mut current = 0u32;
        let mut maximum = 0u32;
        if GetVCPFeatureAndVCPFeatureReply(
            phys.hPhysicalMonitor,
            VCP_CONTRAST,
            None,
            &mut current,
            Some(&mut maximum as *mut u32),
        ) != 0
            && maximum > 0
        {
            let pct = raw_to_percent(current, 0, maximum);
            return Ok((pct, 0, maximum));
        }
    }
    Err(LumarisError::ddc("读取对比度失败"))
}

pub fn write_contrast(id: &str, percent: u32) -> LumarisResult<u32> {
    let percent = percent.min(100);
    let cache = PHYSICAL_CACHE.read();
    let entry = cache
        .get(id)
        .ok_or_else(|| LumarisError::monitor("显示器不存在或已断开"))?;
    let phys = entry.physical;
    drop(cache);

    unsafe {
        let mut current = 0u32;
        let mut maximum = 0u32;
        if GetVCPFeatureAndVCPFeatureReply(
            phys.hPhysicalMonitor,
            VCP_CONTRAST,
            None,
            &mut current,
            Some(&mut maximum as *mut u32),
        ) != 0
            && maximum > 0
        {
            let raw = percent_to_raw(percent, 0, maximum);
            if SetVCPFeature(phys.hPhysicalMonitor, VCP_CONTRAST, raw) != 0 {
                return Ok(percent);
            }
        }
    }
    Err(LumarisError::ddc("写入对比度失败"))
}

// —— 系统消息子类化 ——

static PREV_WNDPROC: Lazy<RwLock<Option<WNDPROC>>> = Lazy::new(|| RwLock::new(None));
static SYS_TX: Lazy<RwLock<Option<std::sync::mpsc::Sender<SystemEvent>>>> =
    Lazy::new(|| RwLock::new(None));

#[derive(Debug, Clone, Copy)]
pub enum SystemEvent {
    DisplayChange,
    DeviceChange,
    PowerSuspend,
    PowerResume,
    DpiChanged,
    SettingChange,
    ThemeChanged,
    TaskbarCreated,
}

pub fn set_system_event_sender(tx: std::sync::mpsc::Sender<SystemEvent>) {
    *SYS_TX.write() = Some(tx);
}

pub fn install_system_message_hook(hwnd_raw: isize) -> LumarisResult<()> {
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        return Err(LumarisError::message("无效 HWND"));
    }
    unsafe {
        let prev = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
        *PREV_WNDPROC.write() = Some(std::mem::transmute(prev));
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            subclass_proc as *const () as usize as isize,
        );
    }
    tracing::info!("系统消息钩子已安装");
    Ok(())
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_msg = {
        use windows::Win32::UI::WindowsAndMessaging::RegisterWindowMessageW;
        RegisterWindowMessageW(windows::core::w!("TaskbarCreated"))
    };

    match msg {
        WM_DISPLAYCHANGE => notify(SystemEvent::DisplayChange),
        WM_DEVICECHANGE => notify(SystemEvent::DeviceChange),
        WM_POWERBROADCAST => {
            const PBT_APMSUSPEND: usize = 0x0004;
            const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
            const PBT_APMRESUMESUSPEND: usize = 0x0007;
            match wparam.0 {
                PBT_APMSUSPEND => notify(SystemEvent::PowerSuspend),
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => notify(SystemEvent::PowerResume),
                _ => {}
            }
        }
        WM_DPICHANGED => notify(SystemEvent::DpiChanged),
        WM_SETTINGCHANGE => notify(SystemEvent::SettingChange),
        WM_THEMECHANGED => notify(SystemEvent::ThemeChanged),
        m if m == taskbar_msg => notify(SystemEvent::TaskbarCreated),
        _ => {}
    }

    let prev = *PREV_WNDPROC.read();
    if let Some(proc) = prev {
        CallWindowProcW(proc, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn notify(ev: SystemEvent) {
    if let Some(tx) = SYS_TX.read().as_ref() {
        let _ = tx.send(ev);
    }
}

pub fn open_path_in_explorer(path: &str) -> LumarisResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("explorer.exe")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| LumarisError::message(format!("打开目录失败: {e}")))?;
    Ok(())
}

pub fn position_window_bottom_right(
    hwnd_raw: isize,
    work: &WorkArea,
    width_px: i32,
    height_px: i32,
    margin: i32,
) -> LumarisResult<()> {
    let hwnd = HWND(hwnd_raw as *mut _);
    let x = work.right - width_px - margin;
    let y = work.bottom - height_px - margin;
    let x = x.max(work.left + margin);
    let y = y.max(work.top + margin);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width_px,
            height_px,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    let _ = (width_px, height_px); // size may already be set by Tauri
    Ok(())
}

