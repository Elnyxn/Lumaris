//! ID-addressed brightness memory, independent of the current list order.

use super::device::{MonitorInfo, MonitorStatus};
use crate::settings::{BrightnessPresetMode, BrightnessPresets};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum PresetError {
    Disabled,
    NoWritableMonitors,
    NotSaved,
    NoMatchingMonitors,
    InvalidBrightness,
}

impl PresetError {
    pub fn message(&self, locale: &str) -> &'static str {
        match (self, locale.starts_with("en")) {
            (Self::Disabled, false) => "请先启用亮度模式",
            (Self::Disabled, true) => "Enable brightness modes first",
            (Self::NoWritableMonitors, false) => "没有可保存的显示器",
            (Self::NotSaved, false) => "请先保存该模式",
            (Self::NoMatchingMonitors, false) => "该模式的显示器未连接或不可调节",
            (Self::NoWritableMonitors, true) => "No adjustable displays to save",
            (Self::NotSaved, true) => "Save this mode first",
            (Self::NoMatchingMonitors, true) => "No saved displays are currently adjustable",
            (Self::InvalidBrightness, false) => "亮度须在 0–100% 之间",
            (Self::InvalidBrightness, true) => "Brightness must be between 0 and 100%",
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

/// Save editor values without changing hardware. Validate the whole selection
/// before merging, and retain existing entries for unplugged displays.
pub fn save_values(
    presets: &mut BrightnessPresets,
    mode: BrightnessPresetMode,
    monitors: &[MonitorInfo],
    values: &HashMap<String, u32>,
) -> Result<(), PresetError> {
    if values.is_empty() {
        return Err(PresetError::NoWritableMonitors);
    }
    for (id, brightness) in values {
        if *brightness > 100 {
            return Err(PresetError::InvalidBrightness);
        }
        if !monitors
            .iter()
            .any(|monitor| monitor.id == *id && writable(monitor))
        {
            return Err(PresetError::NoMatchingMonitors);
        }
    }
    presets
        .get_mut(mode)
        .extend(values.iter().map(|(id, value)| (id.clone(), *value)));
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

    #[test]
    fn editor_values_save_without_using_current_brightness_and_reject_invalid_batches() {
        let mut presets = BrightnessPresets::default();
        let monitors = [monitor("a", 80), monitor("b", 70)];
        let values = HashMap::from([("a".into(), 15), ("b".into(), 25)]);
        save_values(
            &mut presets,
            BrightnessPresetMode::Night,
            &monitors,
            &values,
        )
        .unwrap();
        assert_eq!(presets.night["a"], 15);
        assert_eq!(presets.night["b"], 25);
        // Re-save just a: b is still remembered while unplugged.
        save_values(
            &mut presets,
            BrightnessPresetMode::Night,
            &monitors[..1],
            &HashMap::from([("a".into(), 20)]),
        )
        .unwrap();
        assert_eq!(presets.night["b"], 25);
        let before = presets.clone();
        for invalid in [
            HashMap::from([("a".into(), 101)]),
            HashMap::from([("unknown".into(), 20), ("a".into(), 30)]),
        ] {
            assert!(save_values(
                &mut presets,
                BrightnessPresetMode::Night,
                &monitors,
                &invalid
            )
            .is_err());
            assert_eq!(presets, before);
        }
    }
}
