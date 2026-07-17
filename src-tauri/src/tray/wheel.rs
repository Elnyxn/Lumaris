//! 托盘图标悬停时滚轮调节亮度（Windows 低级鼠标钩子）
//!
//! Tauri TrayIconEvent 无 Scroll 变体；用 Enter/Leave 跟踪悬停，
//! WH_MOUSE_LL 捕获 WM_MOUSEWHEEL。

#![cfg(windows)]

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT, WH_MOUSE_LL,
    WM_MOUSEWHEEL,
};

static TRAY_HOVER: AtomicBool = AtomicBool::new(false);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static WHEEL_TX: Mutex<Option<Sender<i32>>> = Mutex::new(None);
static LAST_WHEEL: Mutex<Option<Instant>> = Mutex::new(None);

/// 滚轮节流：略放宽，保证连续滚动跟手
const WHEEL_MIN_INTERVAL: Duration = Duration::from_millis(12);

pub fn set_tray_hover(hover: bool) {
    TRAY_HOVER.store(hover, Ordering::SeqCst);
}

pub fn is_tray_hover() -> bool {
    TRAY_HOVER.load(Ordering::SeqCst)
}

/// 安装全局低级鼠标钩子；`tx` 收到正=增亮、负=减亮（单位：步数）
pub fn install(tx: Sender<i32>) -> Result<(), String> {
    *WHEEL_TX.lock() = Some(tx);
    // 已安装则复用
    if HOOK.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }
    unsafe {
        let h = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_ll_proc),
            Some(HINSTANCE::default()),
            0,
        )
        .map_err(|e| format!("SetWindowsHookEx WH_MOUSE_LL 失败: {e}"))?;
        HOOK.store(h.0 as isize, Ordering::SeqCst);
    }
    tracing::info!("托盘滚轮钩子已安装");
    Ok(())
}

pub fn uninstall() {
    let raw = HOOK.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(raw as *mut _));
        }
    }
    *WHEEL_TX.lock() = None;
    TRAY_HOVER.store(false, Ordering::SeqCst);
}

unsafe extern "system" fn mouse_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && TRAY_HOVER.load(Ordering::SeqCst) && wparam.0 == WM_MOUSEWHEEL as usize {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        // mouseData 高字：滚轮增量，±120 为一格
        let delta = ((info.mouseData >> 16) as i16) as i32;
        if delta != 0 {
            let mut notches = delta / 120;
            if notches == 0 {
                notches = delta.signum();
            }
            // 一次事件最多 ±3 步，防止触控板爆发
            notches = notches.clamp(-3, 3);
            // 节流
            let now = Instant::now();
            let mut last = LAST_WHEEL.lock();
            let ok = match *last {
                Some(t) => now.duration_since(t) >= WHEEL_MIN_INTERVAL,
                None => true,
            };
            if ok {
                *last = Some(now);
                if let Some(tx) = WHEEL_TX.lock().as_ref() {
                    let _ = tx.send(notches);
                }
            }
        }
    }
    let raw = HOOK.load(Ordering::SeqCst);
    let hhk = if raw == 0 {
        None
    } else {
        Some(HHOOK(raw as *mut _))
    };
    CallNextHookEx(hhk, code, wparam, lparam)
}
