//! 系统消息监听线程：将平台事件转发到应用

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use crate::platform::{self, SystemEvent};

#[derive(Debug, Clone, Copy)]
pub enum AppSystemEvent {
    DisplayChange,
    DeviceChange,
    PowerSuspend,
    PowerResume,
    DpiChanged,
    SettingChange,
    ThemeChanged,
    TaskbarCreated,
}

pub struct SystemMessageHub {
    rx: Receiver<AppSystemEvent>,
    tx: Sender<AppSystemEvent>,
}

impl SystemMessageHub {
    pub fn create() -> Self {
        let (tx, rx) = mpsc::channel();
        #[cfg(windows)]
        {
            let (stx, srx) = mpsc::channel::<SystemEvent>();
            platform::set_system_event_sender(stx);
            let app_tx = tx.clone();
            thread::Builder::new()
                .name("lumaris-sysmsg".into())
                .spawn(move || {
                    // 防抖缓冲
                    let mut pending: Option<AppSystemEvent> = None;
                    let mut last = std::time::Instant::now();
                    loop {
                        match srx.recv_timeout(Duration::from_millis(200)) {
                            Ok(ev) => {
                                let mapped = map_event(ev);
                                // 显示变化类防抖
                                match mapped {
                                    AppSystemEvent::DisplayChange
                                    | AppSystemEvent::DeviceChange => {
                                        pending = Some(mapped);
                                        last = std::time::Instant::now();
                                    }
                                    other => {
                                        let _ = app_tx.send(other);
                                    }
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Some(p) = pending.take() {
                                    if last.elapsed() >= Duration::from_millis(600) {
                                        let _ = app_tx.send(p);
                                    } else {
                                        pending = Some(p);
                                    }
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })
                .ok();
        }
        Self { rx, tx }
    }

    pub fn try_recv(&self) -> Option<AppSystemEvent> {
        self.rx.try_recv().ok()
    }

    /// 返回连接到当前 Hub 的发送端，供平台桥接或测试注入事件。
    pub fn sender(&self) -> Sender<AppSystemEvent> {
        self.tx.clone()
    }
}

#[cfg(windows)]
fn map_event(ev: SystemEvent) -> AppSystemEvent {
    match ev {
        SystemEvent::DisplayChange => AppSystemEvent::DisplayChange,
        SystemEvent::DeviceChange => AppSystemEvent::DeviceChange,
        SystemEvent::PowerSuspend => AppSystemEvent::PowerSuspend,
        SystemEvent::PowerResume => AppSystemEvent::PowerResume,
        SystemEvent::DpiChanged => AppSystemEvent::DpiChanged,
        SystemEvent::SettingChange => AppSystemEvent::SettingChange,
        SystemEvent::ThemeChanged => AppSystemEvent::ThemeChanged,
        SystemEvent::TaskbarCreated => AppSystemEvent::TaskbarCreated,
    }
}

#[cfg(windows)]
pub fn install_hook_for_window(hwnd: isize) -> crate::error::LumarisResult<()> {
    platform::install_system_message_hook(hwnd)
}

#[cfg(not(windows))]
pub fn install_hook_for_window(_hwnd: isize) -> crate::error::LumarisResult<()> {
    Ok(())
}
