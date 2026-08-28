#[cfg(windows)]
pub mod windows;

#[cfg(any(windows, test))]
pub mod sdr_values;

#[cfg(windows)]
pub mod wmi_backlight;

#[cfg(windows)]
pub mod sdr_white_level;

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub mod stub;

#[cfg(not(windows))]
pub use stub::*;
