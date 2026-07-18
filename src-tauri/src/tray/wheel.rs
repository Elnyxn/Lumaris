//! 托盘图标悬停时滚轮调节亮度（Windows 低级鼠标钩子）
//!
//! Tauri TrayIconEvent 无 Scroll 变体；用 Enter/Move 更新图标矩形，
//! Leave 清除；WH_MOUSE_LL 捕获 WM_MOUSEWHEEL，并按事件坐标做点落检测。
//!
//! 绝不能只靠 sticky `hover` 布尔：远程桌面 / 任务栏重排时 Leave 常丢失，
//! 否则会变成「全局滚轮=调亮度」，并连带 quiet OSD 弹出浮窗、抢走设置页滚动。

#![cfg(windows)]

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT, WH_MOUSE_LL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL,
};

/// 软提示：最近是否在托盘区域（仅辅助；真正放行看矩形点落）
static TRAY_HOVER: AtomicBool = AtomicBool::new(false);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static WHEEL_TX: Mutex<Option<Sender<i32>>> = Mutex::new(None);
static LAST_WHEEL: Mutex<Option<Instant>> = Mutex::new(None);
/// 托盘图标物理像素矩形 (left, top, right, bottom)；None = 未悬停
static ICON_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

/// 滚轮节流：略放宽，保证连续滚动跟手
const WHEEL_MIN_INTERVAL: Duration = Duration::from_millis(12);
/// 命中矩形外扩（物理像素），避免边沿漏检
const HIT_PAD: i32 = 2;

pub fn set_tray_hover(hover: bool) {
    TRAY_HOVER.store(hover, Ordering::SeqCst);
    if !hover {
        *ICON_RECT.lock() = None;
    }
}

pub fn is_tray_hover() -> bool {
    TRAY_HOVER.load(Ordering::SeqCst) && ICON_RECT.lock().is_some()
}

/// 由托盘 Enter/Move 更新图标物理矩形
pub fn update_icon_rect(left: i32, top: i32, width: u32, height: u32) {
    if width == 0 || height == 0 {
        set_tray_hover(false);
        return;
    }
    let right = left.saturating_add(width as i32);
    let bottom = top.saturating_add(height as i32);
    *ICON_RECT.lock() = Some((left, top, right, bottom));
    TRAY_HOVER.store(true, Ordering::SeqCst);
}

/// 从 Tauri `Rect`（Windows 托盘上报为物理坐标）更新
pub fn update_icon_rect_from_tauri(rect: &tauri::Rect) {
    // Physical 原样；若偶发 Logical，scale=1.0 时数值与任务栏物理像素一致（100% DPI）
    // 更高 DPI 下 tray-icon 给的是物理 RECT，to_physical(1.0) 对 Physical 变体直接返回
    let pos = rect.position.to_physical::<i32>(1.0);
    let size = rect.size.to_physical::<u32>(1.0);
    update_icon_rect(pos.x, pos.y, size.width, size.height);
}

fn point_in_icon(x: i32, y: i32) -> bool {
    let Some((l, t, r, b)) = *ICON_RECT.lock() else {
        return false;
    };
    x >= l - HIT_PAD && x < r + HIT_PAD && y >= t - HIT_PAD && y < b + HIT_PAD
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
    set_tray_hover(false);
}

unsafe extern "system" fn mouse_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let px = info.pt.x;
        let py = info.pt.y;
        let msg = wparam.0;

        // 仅在“认为悬停中”时做点落检测，避免全局 mousemove 常驻加锁
        if TRAY_HOVER.load(Ordering::SeqCst)
            && (msg == WM_MOUSEMOVE as usize || msg == WM_MOUSEWHEEL as usize)
            && !point_in_icon(px, py)
        {
            // 光标离开图标：立刻清除（补上丢失的 Leave，含远程桌面）
            set_tray_hover(false);
        }

        if msg == WM_MOUSEWHEEL as usize && point_in_icon(px, py) {
            TRAY_HOVER.store(true, Ordering::SeqCst);
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
    }
    let raw = HOOK.load(Ordering::SeqCst);
    let hhk = if raw == 0 {
        None
    } else {
        Some(HHOOK(raw as *mut _))
    };
    CallNextHookEx(hhk, code, wparam, lparam)
}
