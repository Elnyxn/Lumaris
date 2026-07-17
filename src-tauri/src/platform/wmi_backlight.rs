//! 笔记本内屏亮度：WMI / ACPI 背光
//!
//! 外接屏：DDC/CI (VCP 0x10)
//! 笔记本自带屏：通常走 `root\wmi` 的
//!   - WmiMonitorBrightness（读）
//!   - WmiMonitorBrightnessMethods.WmiSetBrightness（写）
//!
//! 「仅第二屏幕」时内屏可能不在枚举列表里，但 WMI 实例仍在。
//! 必须用 InstanceName 硬件 ID 与 DeviceID 精确匹配，禁止绑到外接屏。

use crate::error::{LumarisError, LumarisResult};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Deserialize;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static WMI_AVAILABLE: AtomicBool = AtomicBool::new(false);
static WMI_INIT: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

#[derive(Debug, Deserialize)]
#[serde(rename = "WmiMonitorBrightness")]
#[serde(rename_all = "PascalCase")]
struct WmiMonitorBrightness {
    instance_name: Option<String>,
    current_brightness: Option<u8>,
}

/// 一次探测结果：亮度 + 可匹配的硬件 ID 列表
#[derive(Debug, Clone)]
pub struct WmiProbe {
    pub percent: u32,
    /// 从 InstanceName 解析出的键，如 `BOE0985`
    pub hardware_keys: Vec<String>,
    pub instance_names: Vec<String>,
}

/// 从路径/InstanceName 提取 `DISPLAY\\XXXX` 硬件键
pub fn extract_display_hw_key(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let u = s.to_uppercase().replace('#', "\\");
    // DISPLAY\BOE0985\... 或 \\?\DISPLAY\BOE0985\... 或 DISPLAY#BOE0985#...
    let marker = "DISPLAY\\";
    let idx = u.find(marker)?;
    let rest = &u[idx + marker.len()..];
    let key = rest
        .split(|c| c == '\\' || c == '&' || c == '_' || c == ' ' || c == '\0')
        .next()?
        .trim();
    if key.len() < 3 || key == "DEFAULT_MONITOR" {
        return None;
    }
    Some(key.to_string())
}

/// 设备路径是否对得上 WMI 实例（「仅第二屏幕」时外接 HKC 对不上 BOE）
pub fn device_matches_wmi(device_id: &str, device_name: &str, description: &str, probe: &WmiProbe) -> bool {
    let mut keys = Vec::new();
    if let Some(k) = extract_display_hw_key(device_id) {
        keys.push(k);
    }
    if let Some(k) = extract_display_hw_key(device_name) {
        keys.push(k);
    }
    if let Some(k) = extract_display_hw_key(description) {
        keys.push(k);
    }
    // DeviceID 有时是 \\?\DISPLAY#BOE0985#5&...
    let blob = format!("{device_id}|{device_name}|{description}").to_uppercase();
    for hk in &probe.hardware_keys {
        if keys.iter().any(|k| k == hk) {
            return true;
        }
        // 宽松：路径整串包含硬件键
        if blob.contains(hk) {
            return true;
        }
    }
    // InstanceName 整串匹配
    for inst in &probe.instance_names {
        let iu = inst.to_uppercase();
        if !device_id.is_empty() {
            let du = device_id.to_uppercase().replace('#', "\\");
            if iu.contains(&du) || du.contains(&iu.replace('_', "\\")) {
                return true;
            }
        }
    }
    false
}

pub fn probe_wmi() -> Option<WmiProbe> {
    match query_wmi_rows() {
        Ok(rows) if !rows.is_empty() => {
            let mut hardware_keys = Vec::new();
            let mut instance_names = Vec::new();
            let mut percent = 0u32;
            for r in rows {
                if let Some(p) = r.current_brightness {
                    percent = u32::from(p).min(100);
                }
                if let Some(name) = r.instance_name {
                    if let Some(k) = extract_display_hw_key(&name) {
                        if !hardware_keys.contains(&k) {
                            hardware_keys.push(k);
                        }
                    }
                    instance_names.push(name);
                }
            }
            WMI_AVAILABLE.store(true, Ordering::SeqCst);
            *WMI_INIT.lock() = true;
            tracing::info!(
                percent,
                keys = ?hardware_keys,
                "检测到笔记本 WMI/ACPI 背光"
            );
            Some(WmiProbe {
                percent,
                hardware_keys,
                instance_names,
            })
        }
        Ok(_) => {
            WMI_AVAILABLE.store(false, Ordering::SeqCst);
            *WMI_INIT.lock() = true;
            None
        }
        Err(e) => {
            WMI_AVAILABLE.store(false, Ordering::SeqCst);
            *WMI_INIT.lock() = true;
            tracing::debug!(error = %e, "WMI 背光不可用（台式机或权限）");
            None
        }
    }
}

/// 兼容旧调用
pub fn probe_wmi_brightness() -> Option<u32> {
    probe_wmi().map(|p| p.percent)
}

pub fn is_wmi_available() -> bool {
    if !*WMI_INIT.lock() {
        let _ = probe_wmi();
    }
    WMI_AVAILABLE.load(Ordering::SeqCst)
}

fn query_wmi_rows() -> LumarisResult<Vec<WmiMonitorBrightness>> {
    let com = wmi::COMLibrary::new()
        .map_err(|e| LumarisError::ddc(format!("COM 初始化失败: {e}")))?;
    let wmi = wmi::WMIConnection::with_namespace_path("root\\wmi", com)
        .map_err(|e| LumarisError::ddc(format!("连接 root\\wmi 失败: {e}")))?;

    wmi.raw_query("SELECT InstanceName, CurrentBrightness FROM WmiMonitorBrightness")
        .map_err(|e| LumarisError::ddc(format!("查询 WmiMonitorBrightness 失败: {e}")))
}

pub fn read_brightness() -> LumarisResult<u32> {
    let rows = query_wmi_rows()?;
    let pct = rows
        .into_iter()
        .find_map(|r| r.current_brightness)
        .ok_or_else(|| LumarisError::ddc("未找到 WMI 亮度实例"))?;
    Ok(u32::from(pct).min(100))
}

/// 写入 0–100；Timeout=1 秒（WMI 参数单位为秒）
pub fn write_brightness(percent: u32) -> LumarisResult<()> {
    let percent = percent.min(100);
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $ok=$false; \
         try {{ \
           $list=@(Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop); \
           foreach($m in $list){{ \
             $null=Invoke-CimMethod -InputObject $m -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness={percent}}}; \
             $ok=$true \
           }} \
         }} catch {{ }} \
         if(-not $ok){{ \
           $list=@(Get-WmiObject -Namespace root\\wmi -Class WmiMonitorBrightnessMethods -ErrorAction Stop); \
           if(-not $list){{ throw 'no WmiMonitorBrightnessMethods' }}; \
           foreach($m in $list){{ $null=$m.WmiSetBrightness(1,{percent}) }}; \
           $ok=$true \
         }}; \
         if(-not $ok){{ throw 'WmiSetBrightness failed' }}"
    );

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| LumarisError::ddc(format!("启动 PowerShell 失败: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err2 = String::from_utf8_lossy(&output.stdout);
        return Err(LumarisError::ddc(format!(
            "WmiSetBrightness 失败: {} {}",
            err.trim(),
            err2.trim()
        )));
    }
    WMI_AVAILABLE.store(true, Ordering::SeqCst);
    Ok(())
}
