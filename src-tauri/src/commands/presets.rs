//! Brightness preset IPC. Hardware writes stay on the existing monitor worker.

use crate::events;
use crate::monitor::presets;
use crate::settings::{save_config, AppConfig, BrightnessPresetMode};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn save_brightness_preset(
    app: AppHandle,
    state: State<AppState>,
    mode: BrightnessPresetMode,
) -> Result<AppConfig, String> {
    let monitors = state.monitors.list();
    let mut current = state.config.write();
    let mut next = current.clone();
    presets::capture(&mut next.brightness_presets, mode, &monitors)
        .map_err(|error| error.message(&next.locale).to_string())?;
    next.validate_mut();
    save_config(&next).map_err(|error| error.user_message())?;
    *current = next.clone();
    drop(current);
    let _ = app.emit(events::EVT_SETTINGS_CHANGED, &next);
    Ok(next)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPresetResult {
    pub queued: usize,
    pub unavailable: usize,
}

#[tauri::command]
pub fn apply_brightness_preset(
    app: AppHandle,
    state: State<AppState>,
    mode: BrightnessPresetMode,
) -> Result<ApplyPresetResult, String> {
    let cfg = state.config.read().clone();
    let plan = presets::plan(&cfg.brightness_presets, mode, &state.monitors.list())
        .map_err(|error| error.message(&cfg.locale).to_string())?;
    for (id, brightness) in &plan.writes {
        state.monitors.set_brightness(id, *brightness, true);
    }
    state.schedule_save();
    let _ = app.emit(events::EVT_MONITORS_CHANGED, state.monitors.list());
    // queued is not a hardware acknowledgment: failures use operation-result.
    Ok(ApplyPresetResult {
        queued: plan.writes.len(),
        unavailable: plan.unavailable,
    })
}
