//! 窗口 Acrylic / 透明背景

use crate::error::LumarisResult;
use tauri::{Runtime, WebviewWindow};

pub fn apply_backdrop<R: Runtime>(window: &WebviewWindow<R>) -> LumarisResult<()> {
    #[cfg(windows)]
    {
        let hwnd = window.hwnd().map_err(|e| {
            crate::error::LumarisError::message(format!("获取 HWND 失败: {e}"))
        })?;
        crate::platform::apply_acrylic_backdrop(hwnd.0 as isize)?;
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
    Ok(())
}

pub fn prepare_transparent_window<R: Runtime>(window: &WebviewWindow<R>) {
    // 关系统阴影，避免外圈“浏览器卡片”光晕
    let _ = window.set_shadow(false);
    let _ = window.set_ignore_cursor_events(false);
}
