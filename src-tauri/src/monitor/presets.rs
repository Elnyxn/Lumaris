//! ID-addressed brightness memory, independent of the current list order.

use super::device::{MonitorInfo, MonitorStatus};
use crate::settings::{BrightnessPresetMode, BrightnessPresets};

#[derive(Debug, PartialEq, Eq)]
pub enum PresetError {
    NoWritableMonitors,
    NotSaved,
    NoMatchingMonitors,
}

impl PresetError {
    pub fn message(&self, locale: &str) -> &'static str {
        match (self, locale.starts_with("en")) {
            (Self::NoWritableMonitors, false) => "没有可保存的显示器",
            (Self::NotSaved, false) => "请先保存该模式",
            (Self::NoMatchingMonitors, false) => "该模式的显示器未连接或不可调节",
            (Self::NoWritableMonitors, true) => "No adjustable displays to save",
            (Self::NotSaved, true) => "Save this mode first",
            (Self::NoMatchingMonitors, true) => "No saved displays are currently adjustable",
        }
    }
}

fn writable(monitor: &MonitorInfo) -> bool {
    monitor.is_controllable
        && matches!(
            monitor.status,
            MonitorStatus::Available
                | MonitorStatus::Cached
                | MonitorStatus::ReadFailed
                | MonitorStatus::WriteFailed
        )
}

pub fn capture(
    presets: &mut BrightnessPresets,
    mode: BrightnessPresetMode,
    monitors: &[MonitorInfo],
) -> Result<(), PresetError> {
    let current: Vec<_> = monitors.iter().filter(|m| writable(m)).collect();
    if current.is_empty() {
        return Err(PresetError::NoWritableMonitors);
    }
    // Re-saving one connected screen must retain the unplugged screen's value.
    let saved = presets.get_mut(mode);
    for monitor in current {
        saved.insert(monitor.id.clone(), monitor.cached_brightness.min(100));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct PresetPlan {
    pub writes: Vec<(String, u32)>,
    pub unavailable: usize,
}

pub fn plan(
    presets: &BrightnessPresets,
    mode: BrightnessPresetMode,
    monitors: &[MonitorInfo],
) -> Result<PresetPlan, PresetError> {
    let saved = presets.get(mode);
    if saved.is_empty() {
        return Err(PresetError::NotSaved);
    }
    let writes: Vec<_> = monitors
        .iter()
        .filter(|m| writable(m))
        .filter_map(|m| {
            saved
                .get(&m.id)
                .map(|value| (m.id.clone(), (*value).min(100)))
        })
        .collect();
    if writes.is_empty() {
        return Err(PresetError::NoMatchingMonitors);
    }
    Ok(PresetPlan {
        unavailable: saved.len().saturating_sub(writes.len()),
        writes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::device::{ControlMethod, WorkArea};

    fn monitor(id: &str, brightness: u32) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            display_name: id.into(),
            user_alias: None,
            description: id.into(),
            current_brightness: brightness,
            cached_brightness: brightness,
            min_brightness: 0,
            max_brightness: 100,
            current_contrast: 50,
            cached_contrast: 50,
            min_contrast: 0,
            max_contrast: 100,
            contrast_controllable: false,
            sdr_brightness: 0,
            sdr_controllable: false,
            control_method: ControlMethod::VcpCode10,
            status: MonitorStatus::Available,
            is_primary: false,
            is_external: true,
            is_selected: false,
            is_controllable: true,
            work_area: WorkArea::default(),
            dpi: 96,
        }
    }

    #[test]
    fn day_and_night_keep_independent_values_and_apply_by_id() {
        let mut presets = BrightnessPresets::default();
        capture(
            &mut presets,
            BrightnessPresetMode::Day,
            &[monitor("a", 80), monitor("b", 65)],
        )
        .unwrap();
        capture(
            &mut presets,
            BrightnessPresetMode::Night,
            &[monitor("a", 20), monitor("b", 30)],
        )
        .unwrap();
        let current = [monitor("b", 1), monitor("a", 2), monitor("new", 3)];
        assert_eq!(
            plan(&presets, BrightnessPresetMode::Day, &current)
                .unwrap()
                .writes,
            vec![("b".into(), 65), ("a".into(), 80)]
        );
        assert_eq!(
            plan(&presets, BrightnessPresetMode::Night, &current)
                .unwrap()
                .writes,
            vec![("b".into(), 30), ("a".into(), 20)]
        );
    }

    #[test]
    fn resaving_retains_unplugged_displays_and_skips_unwritable_ones() {
        let mut presets = BrightnessPresets::default();
        capture(
            &mut presets,
            BrightnessPresetMode::Day,
            &[monitor("a", 80), monitor("b", 65)],
        )
        .unwrap();
        let mut sleeping = monitor("b", 0);
        sleeping.status = MonitorStatus::Sleeping;
        capture(
            &mut presets,
            BrightnessPresetMode::Day,
            &[monitor("a", 90), sleeping.clone()],
        )
        .unwrap();
        assert_eq!(presets.day["b"], 65);
        let current = [monitor("a", 1), sleeping];
        let plan = plan(&presets, BrightnessPresetMode::Day, &current).unwrap();
        assert_eq!(plan.writes, vec![("a".into(), 90)]);
        assert_eq!(plan.unavailable, 1);
    }

    #[test]
    fn empty_capture_is_non_destructive_and_unmatched_apply_fails() {
        let mut presets = BrightnessPresets::default();
        assert_eq!(
            plan(&presets, BrightnessPresetMode::Day, &[]),
            Err(PresetError::NotSaved)
        );
        capture(&mut presets, BrightnessPresetMode::Day, &[monitor("a", 80)]).unwrap();
        let before = presets.clone();
        assert_eq!(
            capture(&mut presets, BrightnessPresetMode::Day, &[]),
            Err(PresetError::NoWritableMonitors)
        );
        assert_eq!(presets, before);
        assert_eq!(
            plan(&presets, BrightnessPresetMode::Day, &[monitor("new", 1)]),
            Err(PresetError::NoMatchingMonitors)
        );
    }
}
