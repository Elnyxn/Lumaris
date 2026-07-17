//! 浮窗定位：固定逻辑尺寸 + 工作区右下角（先定位后显示，避免闪动）

use crate::monitor::device::WorkArea;
use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// 唯一尺寸源（逻辑像素）
pub const FLYOUT_WIDTH: f64 = 360.0;
/// 单显示器 · 仅亮度
pub const FLYOUT_HEIGHT: f64 = 122.0;
/// 单显示器 · 亮度 + 对比度
pub const FLYOUT_HEIGHT_WITH_CONTRAST: f64 = 158.0;
/// 堆叠列表：头+底栏+内边距
const FLYOUT_STACK_CHROME: f64 = 92.0;
/// 堆叠列表：单台行高（名称 + 滑条）
const FLYOUT_STACK_ROW: f64 = 62.0;
const FLYOUT_STACK_GAP: f64 = 6.0;
const FLYOUT_STACK_MAX: f64 = 480.0;
pub const SETTINGS_WIDTH: f64 = 360.0;
pub const SETTINGS_HEIGHT: f64 = 520.0;
const MARGIN_LOGICAL: f64 = 12.0;

/// 多显示器一律上下堆叠，不再左右切换
pub fn flyout_uses_stack(monitor_count: usize) -> bool {
    monitor_count > 1
}

pub fn flyout_height(monitor_count: usize, show_contrast: bool) -> f64 {
    // 多屏堆叠：永不加对比度行高
    if flyout_uses_stack(monitor_count) {
        let n = monitor_count.max(1) as f64;
        let h = FLYOUT_STACK_CHROME + n * FLYOUT_STACK_ROW + (n - 1.0).max(0.0) * FLYOUT_STACK_GAP;
        h.min(FLYOUT_STACK_MAX)
    } else if show_contrast && monitor_count <= 1 {
        FLYOUT_HEIGHT_WITH_CONTRAST
    } else {
        FLYOUT_HEIGHT
    }
}

pub fn size_for_page(page: Option<&str>, show_contrast: bool, monitor_count: usize) -> (f64, f64) {
    match page {
        Some("settings") => (SETTINGS_WIDTH, SETTINGS_HEIGHT),
        _ => (FLYOUT_WIDTH, flyout_height(monitor_count, show_contrast)),
    }
}

pub fn position_flyout<R: Runtime>(
    window: &WebviewWindow<R>,
    work: &WorkArea,
    page: Option<&str>,
    dpi: u32,
    show_contrast: bool,
    monitor_count: usize,
) -> tauri::Result<()> {
    let (width_logical, height_logical) = size_for_page(page, show_contrast, monitor_count);
    // 使用目标屏（主屏）DPI，不用窗口当前 scale（窗口可能还在副屏上）
    let scale = {
        let d = if dpi == 0 { 96 } else { dpi };
        d as f64 / 96.0
    };

    let width_phys = (width_logical * scale).round() as u32;
    let height_phys = (height_logical * scale).round() as u32;
    let margin_phys = (MARGIN_LOGICAL * scale).round() as i32;

    let mut x = work.right - width_phys as i32 - margin_phys;
    let mut y = work.bottom - height_phys as i32 - margin_phys;
    x = x.max(work.left + margin_phys);
    y = y.max(work.top + margin_phys);
    if x + width_phys as i32 > work.right {
        x = (work.right - width_phys as i32 - margin_phys).max(work.left);
    }
    if y + height_phys as i32 > work.bottom {
        y = (work.bottom - height_phys as i32 - margin_phys).max(work.top);
    }

    // 尺寸/位置未变则不动，避免 Windows 闪一下
    let need_size = match window.inner_size() {
        Ok(cur) => {
            (cur.width as i32 - width_phys as i32).abs() > 1
                || (cur.height as i32 - height_phys as i32).abs() > 1
        }
        Err(_) => true,
    };
    if need_size {
        window.set_size(PhysicalSize::new(width_phys, height_phys))?;
    }

    let need_pos = match window.outer_position() {
        Ok(cur) => (cur.x - x).abs() > 1 || (cur.y - y).abs() > 1,
        Err(_) => true,
    };
    if need_pos {
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

pub fn apply_fixed_size<R: Runtime>(
    window: &WebviewWindow<R>,
    page: Option<&str>,
    show_contrast: bool,
    monitor_count: usize,
) -> tauri::Result<()> {
    let (w, h) = size_for_page(page, show_contrast, monitor_count);
    window.set_size(LogicalSize::new(w, h))?;
    Ok(())
}
