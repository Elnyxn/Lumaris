//! 目标显示器选择逻辑（可单测）

use crate::monitor::device::MonitorInfo;
use crate::settings::{AppConfig, TargetMode};

/// 解析控制目标 ID 列表
pub fn resolve_target_ids(
    monitors: &[MonitorInfo],
    cfg: &AppConfig,
    cursor_id: Option<String>,
    selected_id: Option<String>,
) -> Vec<String> {
    if cfg.sync_all || cfg.target_mode == TargetMode::AllSync {
        return monitors
            .iter()
            .filter(|m| {
                m.is_controllable
                    && cfg
                        .monitor_sync_include
                        .get(&m.id)
                        .copied()
                        .unwrap_or(true)
            })
            .map(|m| m.id.clone())
            .collect();
    }

    let id = match cfg.target_mode {
        TargetMode::MouseMonitor => cursor_id
            .or(selected_id)
            .or_else(|| cfg.last_monitor_id.clone())
            .or_else(|| monitors.iter().find(|m| m.is_primary).map(|m| m.id.clone()))
            .or_else(|| {
                monitors
                    .iter()
                    .find(|m| m.is_controllable)
                    .map(|m| m.id.clone())
            }),
        TargetMode::LastUsed => cfg.last_monitor_id.clone().or(selected_id),
        TargetMode::Primary => monitors
            .iter()
            .find(|m| m.is_primary)
            .map(|m| m.id.clone())
            .or(selected_id),
        TargetMode::Fixed => cfg.fixed_monitor_id.clone().or(selected_id),
        TargetMode::AllSync => None,
    };

    // 校验存在
    id.into_iter()
        .filter(|i| monitors.iter().any(|m| &m.id == i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::device::{ControlMethod, MonitorStatus, WorkArea};

    fn mon(id: &str, primary: bool, controllable: bool) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            display_name: id.into(),
            user_alias: None,
            description: id.into(),
            current_brightness: 50,
            cached_brightness: 50,
            min_brightness: 0,
            max_brightness: 100,
            current_contrast: 50,
            cached_contrast: 50,
            min_contrast: 0,
            max_contrast: 100,
            contrast_controllable: false,
            control_method: if controllable {
                ControlMethod::VcpCode10
            } else {
                ControlMethod::Unsupported
            },
            status: if controllable {
                MonitorStatus::Available
            } else {
                MonitorStatus::Unsupported
            },
            is_primary: primary,
            is_external: !primary,
            is_selected: false,
            is_controllable: controllable,
            work_area: WorkArea::default(),
            dpi: 96,
        }
    }

    #[test]
    fn mouse_fallback_chain() {
        let list = vec![mon("a", true, true), mon("b", false, true)];
        let mut cfg = AppConfig::default();
        cfg.target_mode = TargetMode::MouseMonitor;
        let ids = resolve_target_ids(&list, &cfg, None, None);
        assert_eq!(ids, vec!["a".to_string()]);
    }

    #[test]
    fn sync_all_filters() {
        let list = vec![mon("a", true, true), mon("b", false, false)];
        let mut cfg = AppConfig::default();
        cfg.sync_all = true;
        let ids = resolve_target_ids(&list, &cfg, None, None);
        assert_eq!(ids, vec!["a".to_string()]);
    }
}
