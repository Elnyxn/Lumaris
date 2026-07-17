use crate::error::{LumarisError, LumarisResult};
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;

pub const RUN_VALUE_NAME: &str = "Lumaris";

pub struct StartupManager;

impl StartupManager {
    /// 读取系统实际自启状态（非仅 config）
    pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
        app.autolaunch().is_enabled().unwrap_or(false)
    }

    pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> LumarisResult<bool> {
        if enabled {
            app.autolaunch()
                .enable()
                .map_err(|e| LumarisError::Autostart(format!("启用自启失败: {e}")))?;
            // 补充 --startup 参数到注册表（插件可能不带参数）
            #[cfg(windows)]
            {
                let _ = ensure_run_key_with_startup();
            }
        } else {
            app.autolaunch()
                .disable()
                .map_err(|e| LumarisError::Autostart(format!("禁用自启失败: {e}")))?;
            #[cfg(windows)]
            {
                let _ = remove_run_key();
            }
        }
        let actual = Self::is_enabled(app);
        if enabled != actual {
            // 插件状态与期望不一致时，以注册表为准再验
            #[cfg(windows)]
            {
                let reg = is_run_key_present();
                if enabled && !reg {
                    return Err(LumarisError::Autostart(
                        "自启启用失败：系统启动项未写入".into(),
                    ));
                }
                if !enabled && reg {
                    return Err(LumarisError::Autostart(
                        "自启禁用失败：系统启动项仍存在".into(),
                    ));
                }
                return Ok(reg);
            }
            #[cfg(not(windows))]
            {
                return Ok(actual);
            }
        }
        Ok(actual)
    }
}

#[cfg(windows)]
fn exe_path() -> LumarisResult<String> {
    let p = std::env::current_exe()
        .map_err(|e| LumarisError::Autostart(format!("无法获取程序路径: {e}")))?;
    Ok(p.to_string_lossy().to_string())
}

#[cfg(windows)]
fn ensure_run_key_with_startup() -> LumarisResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
    };

    let path = exe_path()?;
    let cmd = format!("\"{path}\" --startup");
    let value: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
    let name: Vec<u16> = RUN_VALUE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = std::mem::zeroed();
        let sub = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, sub, Some(0), KEY_WRITE, &mut hkey);
        if status.is_err() {
            return Err(LumarisError::Autostart("无法打开 Run 注册表项".into()));
        }
        let set = RegSetValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                value.as_ptr() as *const u8,
                value.len() * 2,
            )),
        );
        let _ = RegCloseKey(hkey);
        if set.is_err() {
            return Err(LumarisError::Autostart("写入 Run 启动项失败".into()));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn remove_run_key() -> LumarisResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_WRITE,
    };
    let name: Vec<u16> = RUN_VALUE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let mut hkey = std::mem::zeroed();
        let sub = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub, Some(0), KEY_WRITE, &mut hkey).is_err() {
            return Ok(());
        }
        let _ = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
        let _ = RegCloseKey(hkey);
    }
    Ok(())
}

#[cfg(windows)]
fn is_run_key_present() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
    };
    let name: Vec<u16> = RUN_VALUE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let mut hkey = std::mem::zeroed();
        let sub = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub, Some(0), KEY_READ, &mut hkey).is_err() {
            return false;
        }
        let mut typ = windows::Win32::System::Registry::REG_VALUE_TYPE::default();
        let mut size = 0u32;
        let q = RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut typ),
            None,
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        q.is_ok()
    }
}
