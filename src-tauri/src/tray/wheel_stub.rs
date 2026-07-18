//! 非 Windows：托盘滚轮 no-op

use std::sync::mpsc::Sender;

pub fn set_tray_hover(_hover: bool) {}
pub fn is_tray_hover() -> bool {
    false
}
pub fn update_icon_rect(_left: i32, _top: i32, _width: u32, _height: u32) {}
pub fn update_icon_rect_from_tauri(_rect: &tauri::Rect) {}
pub fn install(_tx: Sender<i32>) -> Result<(), String> {
    Ok(())
}
pub fn uninstall() {}
