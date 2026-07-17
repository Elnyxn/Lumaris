pub mod menu;

#[cfg(windows)]
pub mod wheel;

#[cfg(not(windows))]
pub mod wheel_stub;

#[cfg(windows)]
pub use wheel as tray_wheel;

#[cfg(not(windows))]
pub use wheel_stub as tray_wheel;

pub use menu::setup_tray;
