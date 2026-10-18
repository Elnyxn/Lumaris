//! 浮窗定位：实际内容高度 + 主屏工作区上限（先定位后显示，避免闪动）

use crate::monitor::device::WorkArea;
use parking_lot::Mutex;
use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// 唯一尺寸源（逻辑像素）
pub const FLYOUT_WIDTH: f64 = 360.0;
/// 单显示器 · 仅亮度
pub const FLYOUT_HEIGHT: f64 = 122.0;
/// 单显示器 · 亮度 + 对比度
pub const FLYOUT_HEIGHT_WITH_CONTRAST: f64 = 158.0;
/// 单屏 SDR 行：32px 控件 + 6px flex 间距
const FLYOUT_SDR_SINGLE_ROW: f64 = 38.0;
/// 多屏 SDR：标签 + 32px 滑块 + grid 间距
const FLYOUT_SDR_STACK_ROW: f64 = 52.0;
/// 堆叠列表：头+底栏+内边距
const FLYOUT_STACK_CHROME: f64 = 92.0;
/// 堆叠列表：单台行高（名称 + 滑条）
const FLYOUT_STACK_ROW: f64 = 62.0;
const FLYOUT_STACK_GAP: f64 = 6.0;
// 单一主窗口：保留最近一次 DOM 测量，后端再次显示或刷新工作区时复用。
// 布局不同则回退估算，前端渲染后会提交新的实际高度。
static CONTENT_HEIGHT: Mutex<Option<(usize, bool, usize, f64)>> = Mutex::new(None);
pub const SETTINGS_WIDTH: f64 = 360.0;
pub const SETTINGS_HEIGHT: f64 = 520.0;
const MARGIN_LOGICAL: f64 = 12.0;

/// 多显示器一律上下堆叠，不再左右切换
pub fn flyout_uses_stack(monitor_count: usize) -> bool {
    monitor_count > 1
}

pub fn flyout_height(monitor_count: usize, show_contrast: bool, sdr_count: usize) -> f64 {
    // 多屏堆叠：永不加对比度行高
    if flyout_uses_stack(monitor_count) {
        let n = monitor_count.max(1) as f64;
        FLYOUT_STACK_CHROME
            + n * FLYOUT_STACK_ROW
            + (n - 1.0).max(0.0) * FLYOUT_STACK_GAP
            + sdr_count as f64 * FLYOUT_SDR_STACK_ROW
    } else {
        let base = if show_contrast && monitor_count <= 1 {
            FLYOUT_HEIGHT_WITH_CONTRAST
        } else {
            FLYOUT_HEIGHT
        };
        base + usize::from(sdr_count > 0) as f64 * FLYOUT_SDR_SINGLE_ROW
    }
}

pub fn set_flyout_content_height(
    height: f64,
    show_contrast: bool,
    monitor_count: usize,
    sdr_count: usize,
) -> Result<(), &'static str> {
    if !height.is_finite() || height <= 0.0 || height > 1_000_000.0 {
        return Err("无效的浮窗内容高度");
    }
    *CONTENT_HEIGHT.lock() = Some((monitor_count, show_contrast, sdr_count, height));
    Ok(())
}

fn dpi_scale(dpi: u32) -> f64 {
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

/// 工作区是物理像素；上限针对整个浮窗，先换算成目标屏的逻辑像素。
fn constrained_height(height: f64, work: &WorkArea, dpi: u32, settings: bool) -> f64 {
    let scale = dpi_scale(dpi);
    let available = f64::from(work.height().max(1));
    let max_physical = if settings {
        (available - 2.0 * MARGIN_LOGICAL * scale).max(1.0)
    } else {
        (available * 0.5).floor().max(1.0)
    };
    height.min(max_physical / scale)
}

pub fn size_for_page(
    page: Option<&str>,
    show_contrast: bool,
    monitor_count: usize,
    sdr_count: usize,
) -> (f64, f64) {
    match page {
        Some("settings") => (SETTINGS_WIDTH, SETTINGS_HEIGHT),
        _ => {
            let measured = CONTENT_HEIGHT
                .lock()
                .filter(|&(n, c, s, _)| (n, c, s) == (monitor_count, show_contrast, sdr_count));
            (
                FLYOUT_WIDTH,
                measured
                    .map(|(_, _, _, h)| h)
                    .unwrap_or_else(|| flyout_height(monitor_count, show_contrast, sdr_count)),
            )
        }
    }
}

pub fn position_flyout<R: Runtime>(
    window: &WebviewWindow<R>,
    work: &WorkArea,
    page: Option<&str>,
    dpi: u32,
    show_contrast: bool,
    monitor_count: usize,
    sdr_count: usize,
) -> tauri::Result<()> {
    let (width_logical, height_logical) =
        size_for_page(page, show_contrast, monitor_count, sdr_count);
    // 使用目标屏（主屏）DPI，不用窗口当前 scale（窗口可能还在副屏上）
    let scale = dpi_scale(dpi);
    let height_logical = constrained_height(height_logical, work, dpi, page == Some("settings"));

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
        // 一像素的欠高也会产生虚假滚动条，尺寸必须精确匹配。
        Ok(cur) => cur.width != width_phys || cur.height != height_phys,
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
    sdr_count: usize,
) -> tauri::Result<()> {
    let (w, h) = size_for_page(page, show_contrast, monitor_count, sdr_count);
    let h = constrained_height(
        h,
        &crate::platform::primary_work_area(),
        crate::platform::primary_dpi(),
        page == Some("settings"),
    );
    window.set_size(LogicalSize::new(w, h))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_display_rows_are_counted() {
        assert_eq!(flyout_height(1, false, 0), 122.0);
        assert_eq!(flyout_height(1, false, 1), 160.0);
        assert_eq!(flyout_height(1, true, 1), 196.0);
    }

    #[test]
    fn stacked_sdr_rows_use_full_grid_height() {
        assert_eq!(flyout_height(2, false, 0), 222.0);
        assert_eq!(flyout_height(2, false, 1), 274.0);
        assert_eq!(flyout_height(2, false, 2), 326.0);
        assert!(flyout_height(12, false, 12) > 560.0);
    }

    #[test]
    fn content_is_capped_only_at_half_the_target_work_area() {
        let work = WorkArea {
            left: 0,
            top: 0,
            right: 3840,
            bottom: 2080,
        };
        // 200% DPI: 2080 physical => 1040 logical => 520 logical cap.
        assert_eq!(constrained_height(326.0, &work, 192, false), 326.0);
        assert_eq!(constrained_height(520.0, &work, 192, false), 520.0);
        assert_eq!(constrained_height(900.0, &work, 192, false), 520.0);
        assert_eq!(constrained_height(900.0, &work, 96, false), 900.0);
        // Settings keep their separate height policy.
        assert_eq!(constrained_height(SETTINGS_HEIGHT, &work, 192, true), 520.0);
    }

    #[test]
    fn fractional_scaling_and_offset_work_areas_keep_the_physical_cap() {
        let work = WorkArea {
            left: -2560,
            top: -100,
            right: 0,
            bottom: 1301,
        };
        let h = constrained_height(2000.0, &work, 144, false);
        assert_eq!((h * 1.5).round() as u32, 700);
        assert_eq!(constrained_height(300.0, &work, 0, false), 300.0);
    }
}
