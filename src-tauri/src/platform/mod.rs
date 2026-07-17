#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub mod wmi_backlight;

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub mod stub;

#[cfg(not(windows))]
pub use stub::*;
