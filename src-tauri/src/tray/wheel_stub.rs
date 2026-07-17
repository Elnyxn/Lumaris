//! 非 Windows：托盘滚轮 no-op

use std::sync::mpsc::Sender;

pub fn set_tray_hover(_hover: bool) {}
pub fn is_tray_hover() -> bool {
    false
}
pub fn install(_tx: Sender<i32>) -> Result<(), String> {
    Ok(())
}
pub fn uninstall() {}
