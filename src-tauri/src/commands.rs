//! Tauri 命令：前端唯一 IPC 入口

use crate::events::{self, BrightnessChangedEvent, OperationResultEvent};
use crate::hotkey::normalize_accelerator;
use crate::settings::{
    logs_dir, save_config, AppConfig, HotkeyConfig, LayoutMode, LogLevelSetting, TargetMode,
    UiSettings, OsdSettings, MAX_MONITOR_ALIAS_LEN, MAX_MONITOR_ID_LEN,
};
use crate::state::AppState;
use crate::window::{apply_backdrop, apply_fixed_size, position_flyout};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const UI_ERROR_MESSAGE_MAX_CHARS: usize = 2048;
const UI_ERROR_CODE_MAX_CHARS: usize = 128;
const UI_ERROR_LIMIT_PER_MINUTE: u32 = 20;

struct UiErrorRateState {
    window_started: Instant,
    accepted: u32,
    dropped: u32,
}

static UI_ERROR_RATE: Lazy<Mutex<UiErrorRateState>> = Lazy::new(|| {
    Mutex::new(UiErrorRateState {
        window_started: Instant::now(),
        accepted: 0,
        dropped: 0,
    })
});

fn ensure_known_monitor(state: &AppState, monitor_id: &str) -> Result<(), String> {
    if monitor_id.trim().is_empty() || monitor_id.chars().count() > MAX_MONITOR_ID_LEN {
        return Err("无效的显示器 ID".into());
    }
    if !state.monitors.list().iter().any(|monitor| monitor.id == monitor_id) {
        return Err("显示器不存在".into());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub config: AppConfig,
    pub monitors: Vec<crate::monitor::MonitorInfo>,
    pub autostart_enabled: bool,
    pub startup_mode: bool,
    pub version: String,
}

#[tauri::command]
pub fn frontend_ready(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    state.mark_frontend_ready();
    tracing::info!("前端已就绪");

    // 应用背景材质
    if let Some(win) = app.get_webview_window("main") {
        if let Err(e) = apply_backdrop(&win) {
            tracing::warn!(error = %e, "Acrylic 背景设置失败，使用 CSS 降级");
        }
        #[cfg(windows)]
        {
            if let Ok(hwnd) = win.hwnd() {
                let _ = crate::window::install_hook_for_window(hwnd.0 as isize);
            }
        }
    }

    // 推送初始状态
    let snap = build_snapshot(&app, &state);
    let _ = app.emit(events::EVT_APP_STATE, &snap);
    let _ = app.emit(events::EVT_FRONTEND_READY_ACK, ());
    let _ = app.emit(events::EVT_MONITORS_CHANGED, &snap.monitors);

    // 非静默启动时确保浮窗可见（后端兜底）
    if !state.is_startup_mode() {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
        }
        let _ = app.emit("ui-show-flyout", serde_json::json!({ "page": "flyout" }));
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_snapshot(app: AppHandle, state: State<AppState>) -> Result<AppSnapshot, String> {
    Ok(build_snapshot(&app, &state))
}

fn build_snapshot(app: &AppHandle, state: &AppState) -> AppSnapshot {
    let config = state.config.read().clone();
    let monitors = state.monitors.list();
    let autostart_enabled = crate::startup::StartupManager::is_enabled(app);
    AppSnapshot {
        config,
        monitors,
        autostart_enabled,
        startup_mode: state.is_startup_mode(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[tauri::command]
pub fn get_monitors(state: State<AppState>) -> Result<Vec<crate::monitor::MonitorInfo>, String> {
    Ok(state.monitors.list())
}

#[tauri::command]
pub fn refresh_monitors(state: State<AppState>) -> Result<(), String> {
    state.monitors.force_refresh();
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBrightnessArgs {
    pub monitor_id: String,
    pub percent: u32,
    pub final_write: bool,
}

#[tauri::command]
pub fn set_brightness(
    app: AppHandle,
    state: State<AppState>,
    args: SetBrightnessArgs,
) -> Result<(), String> {
    ensure_known_monitor(&state, &args.monitor_id)?;
    let percent = args.percent.min(100);
    state
        .monitors
        .set_brightness(&args.monitor_id, percent, args.final_write);

    // 记录最近使用
    {
        let mut cfg = state.config.write();
        if cfg.remember_last_monitor {
            cfg.last_monitor_id = Some(args.monitor_id.clone());
        }
        cfg.cached_brightness
            .insert(args.monitor_id.clone(), percent);
    }
    if args.final_write {
        state.schedule_save();
        // 仅 final 推事件；中间值不广播，避免滚轮/拖动时 UI 被旧事件拉回
        let _ = app.emit(
            events::EVT_BRIGHTNESS_CHANGED,
            BrightnessChangedEvent {
                monitor_id: args.monitor_id,
                brightness: percent,
                cached: true,
                status: "Cached".into(),
            },
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetContrastArgs {
    pub monitor_id: String,
    pub percent: u32,
    pub final_write: bool,
}

#[tauri::command]
pub fn set_contrast(
    app: AppHandle,
    state: State<AppState>,
    args: SetContrastArgs,
) -> Result<(), String> {
    ensure_known_monitor(&state, &args.monitor_id)?;
    let percent = args.percent.min(100);
    state
        .monitors
        .set_contrast(&args.monitor_id, percent, args.final_write);
    {
        let mut cfg = state.config.write();
        cfg.cached_contrast
            .insert(args.monitor_id.clone(), percent);
    }
    if args.final_write {
        state.schedule_save();
        let _ = app.emit(
            events::EVT_CONTRAST_CHANGED,
            events::ContrastChangedEvent {
                monitor_id: args.monitor_id,
                contrast: percent,
                cached: true,
                status: "Cached".into(),
            },
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustBrightnessArgs {
    pub delta_steps: i32,
    pub final_write: bool,
    pub sync_all: Option<bool>,
    pub monitor_id: Option<String>,
}

#[tauri::command]
pub fn adjust_brightness(
    app: AppHandle,
    state: State<AppState>,
    args: AdjustBrightnessArgs,
) -> Result<Vec<OperationResultEvent>, String> {
    let cfg = state.config.read().clone();
    let mut cfg = cfg;
    if let Some(sync) = args.sync_all {
        cfg.sync_all = sync;
    }
    if let Some(id) = args.monitor_id.as_deref() {
        ensure_known_monitor(&state, id)?;
    }
    let ids = args.monitor_id.map(|id| vec![id]);
    let results = state
        .monitors
        .adjust_brightness(&cfg, args.delta_steps, args.final_write, ids);

    let mut events_out = Vec::new();
    for (id, pct) in results {
        if cfg.remember_last_monitor {
            state.config.write().last_monitor_id = Some(id.clone());
        }
        state.config.write().cached_brightness.insert(id.clone(), pct);
        let ev = OperationResultEvent {
            monitor_id: id.clone(),
            success: true,
            brightness: Some(pct),
            error: None,
            kind: "adjust".into(),
        };
        let _ = app.emit(events::EVT_BRIGHTNESS_CHANGED, BrightnessChangedEvent {
            monitor_id: id,
            brightness: pct,
            cached: true,
            status: "Cached".into(),
        });
        events_out.push(ev);
    }
    if args.final_write {
        state.schedule_save();
    }
    // 显示浮窗
    let _ = app.emit("show-flyout", ());
    Ok(events_out)
}

#[tauri::command]
pub fn select_monitor(state: State<AppState>, monitor_id: String) -> Result<(), String> {
    state
        .monitors
        .select(&monitor_id)
        .map_err(|e| e.user_message())?;
    if state.config.read().remember_last_monitor {
        state.config.write().last_monitor_id = Some(monitor_id);
        state.schedule_save();
    }
    Ok(())
}

#[tauri::command]
pub fn select_next_monitor(state: State<AppState>) -> Result<Option<crate::monitor::MonitorInfo>, String> {
    Ok(state.monitors.select_next())
}

#[tauri::command]
pub fn select_prev_monitor(state: State<AppState>) -> Result<Option<crate::monitor::MonitorInfo>, String> {
    Ok(state.monitors.select_prev())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowFlyoutArgs {
    /// 忽略旧客户端的 height，尺寸只由 page 决定
    pub height: Option<f64>,
    pub page: Option<String>,
}

#[tauri::command]
pub fn show_flyout(
    app: AppHandle,
    state: State<AppState>,
    args: ShowFlyoutArgs,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;

    let page = args.page.as_deref().unwrap_or("flyout");
    state.set_current_page(page);
    // 浮窗固定主显示器右下角（亮度目标仍可跟鼠标/选中屏）
    let (work, dpi) = state
        .monitors
        .list()
        .into_iter()
        .find(|m| m.is_primary)
        .map(|m| (m.work_area, m.dpi))
        .unwrap_or_else(|| {
            (
                crate::platform::primary_work_area(),
                crate::platform::primary_dpi(),
            )
        });

    let mon_count = state.monitors.list().len();
    // 对比度仅单屏生效；多屏堆叠不加高度
    let show_c = state.config.read().show_contrast && mon_count <= 1;

    let _ = win.set_ignore_cursor_events(false);

    // 始终按 page 定尺寸（flyout ↔ settings 高度不同）；禁止“已显示就跳过”导致设置页锁死在浮窗高度
    if let Err(e) = position_flyout(&win, &work, Some(page), dpi, show_c, mon_count) {
        tracing::warn!(error = %e, "定位浮窗失败，回退固定尺寸");
        let _ = apply_fixed_size(&win, Some(page), show_c, mon_count);
    }

    let already = win.is_visible().unwrap_or(false);
    if already {
        // 已显示：只改尺寸，不再 show/set_focus（避免调节亮度时闪窗）
        state.set_flyout_open(true);
        return Ok(());
    }

    win.show().map_err(|e| e.to_string())?;
    let _ = win.set_focus();
    state.set_flyout_open(true);
    let _ = app.emit(events::EVT_WINDOW_SHOWN, args.page);
    Ok(())
}

#[tauri::command]
pub fn hide_flyout(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.hide().map_err(|e| e.to_string())?;
    }
    state.set_flyout_open(false);
    let _ = app.emit(events::EVT_WINDOW_HIDDEN, ());
    Ok(())
}

/// 仅按页面切换固定尺寸（不再接受任意宽高）
#[tauri::command]
pub fn resize_flyout(
    app: AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    // 兼容旧前端调用：忽略传入宽高，按当前可见页面语义固定
    let _ = (width, height);
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    // 无法可靠知前端 page 时保持 flyout 尺寸，避免乱跳
    let (show_contrast, mon_count) = app
        .try_state::<AppState>()
        .map(|s| {
            let n = s.monitors.list().len();
            (s.config.read().show_contrast && n <= 1, n)
        })
        .unwrap_or((false, 1));
    apply_fixed_size(&win, Some("flyout"), show_contrast, mon_count)
        .map_err(|e| e.to_string())?;
    let _ = win.set_ignore_cursor_events(false);
    Ok(())
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    Ok(state.config.read().clone())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfigPatch {
    pub step_percent: Option<u32>,
    pub custom_step_percent: Option<u32>,
    pub silent_startup: Option<bool>,
    pub delayed_monitor_init: Option<bool>,
    pub delayed_init_ms: Option<u64>,
    pub target_mode: Option<TargetMode>,
    pub fixed_monitor_id: Option<Option<String>>,
    pub layout_mode: Option<LayoutMode>,
    pub sync_all: Option<bool>,
    pub remember_last_monitor: Option<bool>,
    pub read_brightness_on_start: Option<bool>,
    pub show_contrast: Option<bool>,
    pub locale: Option<String>,
    pub osd: Option<OsdSettings>,
    pub ui: Option<UiSettings>,
    pub log_level: Option<LogLevelSetting>,
}

#[tauri::command]
pub fn update_config(
    app: AppHandle,
    state: State<AppState>,
    patch: UpdateConfigPatch,
) -> Result<AppConfig, String> {
    if let Some(Some(id)) = patch.fixed_monitor_id.as_ref() {
        ensure_known_monitor(&state, id)?;
    }
    {
        let mut cfg = state.config.write();
        if let Some(v) = patch.step_percent {
            cfg.step_percent = v;
        }
        if let Some(v) = patch.custom_step_percent {
            cfg.custom_step_percent = v.clamp(1, 25);
        }
        if let Some(v) = patch.silent_startup {
            cfg.silent_startup = v;
        }
        if let Some(v) = patch.delayed_monitor_init {
            cfg.delayed_monitor_init = v;
        }
        if let Some(v) = patch.delayed_init_ms {
            cfg.delayed_init_ms = v;
        }
        if let Some(v) = patch.target_mode {
            cfg.target_mode = v;
        }
        if let Some(v) = patch.fixed_monitor_id {
            cfg.fixed_monitor_id = v;
        }
        if let Some(v) = patch.layout_mode {
            cfg.layout_mode = v;
        }
        if let Some(v) = patch.sync_all {
            cfg.sync_all = v;
        }
        if let Some(v) = patch.remember_last_monitor {
            cfg.remember_last_monitor = v;
        }
        if let Some(v) = patch.read_brightness_on_start {
            cfg.read_brightness_on_start = v;
        }
        if let Some(v) = patch.show_contrast {
            cfg.show_contrast = v;
        }
        if let Some(v) = patch.locale {
            cfg.locale = v;
        }
        if let Some(v) = patch.osd {
            cfg.osd = v;
        }
        if let Some(v) = patch.ui {
            cfg.ui = v;
        }
        if let Some(v) = patch.log_level {
            cfg.log_level = v;
        }
        cfg.validate_mut();
    }
    state.save_now();
    let cfg = state.config.read().clone();
    // 按当前 locale 刷新托盘文案
    if let Err(e) = crate::tray::menu::setup_tray(&app) {
        tracing::warn!(error = %e, "刷新托盘语言失败");
    }
    let _ = app.emit(events::EVT_SETTINGS_CHANGED, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub fn set_monitor_alias(
    state: State<AppState>,
    monitor_id: String,
    alias: Option<String>,
) -> Result<(), String> {
    ensure_known_monitor(&state, &monitor_id)?;
    if alias
        .as_deref()
        .is_some_and(|value| value.trim().chars().count() > MAX_MONITOR_ALIAS_LEN)
    {
        return Err(format!("显示器别名不能超过 {MAX_MONITOR_ALIAS_LEN} 个字符"));
    }
    {
        let mut cfg = state.config.write();
        match alias {
            Some(a) if !a.trim().is_empty() => {
                cfg.monitor_aliases
                    .insert(monitor_id.clone(), a.trim().to_string());
            }
            _ => {
                cfg.monitor_aliases.remove(&monitor_id);
            }
        }
    }
    // 更新内存列表
    let mut list = state.monitors.list();
    for m in list.iter_mut() {
        if m.id == monitor_id {
            m.user_alias = state.config.read().monitor_aliases.get(&monitor_id).cloned();
        }
    }
    let cfg = state.config.read().clone();
    state.monitors.apply_monitors(list, &cfg);
    state.schedule_save();
    Ok(())
}

#[tauri::command]
pub fn set_monitor_sync_include(
    state: State<AppState>,
    monitor_id: String,
    include: bool,
) -> Result<(), String> {
    ensure_known_monitor(&state, &monitor_id)?;
    state
        .config
        .write()
        .monitor_sync_include
        .insert(monitor_id, include);
    state.schedule_save();
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetHotkeyArgs {
    pub action: String,
    pub accelerator: Option<String>,
}

#[tauri::command]
pub fn set_hotkey(
    app: AppHandle,
    state: State<AppState>,
    args: SetHotkeyArgs,
) -> Result<Option<String>, String> {
    let action = args.action;
    let valid_actions = [
        "increase",
        "decrease",
        "toggle_flyout",
        "prev_monitor",
        "next_monitor",
        "sync_increase",
        "sync_decrease",
    ];
    if !valid_actions.contains(&action.as_str()) {
        return Err("未知快捷键动作".into());
    }

    // 应用内冲突
    if let Some(ref accel) = args.accelerator {
        let norm = normalize_accelerator(accel).map_err(|e| e.user_message())?;
        let cfg = state.config.read();
        for (a, existing) in cfg.hotkeys.iter_actions() {
            if a != action {
                if let Some(ex) = existing {
                    if let Ok(en) = normalize_accelerator(ex) {
                        if en == norm {
                            return Err("与其他动作的快捷键冲突".into());
                        }
                    }
                }
            }
        }
        drop(cfg);
    }

    let registered = state
        .hotkeys
        .update_action(&app, &action, args.accelerator.as_deref())
        .map_err(|e| e.user_message())?;

    state
        .config
        .write()
        .hotkeys
        .set_action(&action, registered.clone());
    state.save_now();
    Ok(registered)
}

#[tauri::command]
pub fn reset_hotkeys(app: AppHandle, state: State<AppState>) -> Result<HotkeyConfig, String> {
    let defaults = HotkeyConfig::defaults();
    let previous = state.config.read().hotkeys.clone();
    let results = state.hotkeys.register_all(&app, &defaults);
    let failures: Vec<_> = results
        .iter()
        .filter_map(|(action, result)| result.as_ref().err().map(|error| (action, error)))
        .collect();
    if !failures.is_empty() {
        for (action, error) in failures {
            tracing::warn!(action, error = %error, "默认快捷键注册失败");
        }
        let restored = state.hotkeys.register_all(&app, &previous);
        let restore_failed = restored.iter().any(|(_, result)| result.is_err());
        if restore_failed {
            let registered = state.hotkeys.registered_map();
            let mut actual = previous.clone();
            for (action, _) in previous.iter_actions() {
                actual.set_action(action, registered.get(action).cloned());
            }
            state.config.write().hotkeys = actual;
            state.save_now();
            let _ = app.emit(events::EVT_SETTINGS_CHANGED, state.config.read().clone());
            tracing::error!("恢复原快捷键时部分注册失败，配置已同步为实际注册状态");
            return Err("默认快捷键注册失败，部分原设置也无法恢复".into());
        }
        return Err("默认快捷键注册失败，已恢复原设置".into());
    }
    state.config.write().hotkeys = defaults.clone();
    state.save_now();
    Ok(defaults)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, state: State<AppState>, enabled: bool) -> Result<bool, String> {
    let actual = crate::startup::StartupManager::set_enabled(&app, enabled)
        .map_err(|e| e.user_message())?;
    state.config.write().autostart = actual;
    state.save_now();
    Ok(actual)
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    Ok(crate::startup::StartupManager::is_enabled(&app))
}

#[tauri::command]
pub fn open_logs_dir() -> Result<(), String> {
    let dir = logs_dir().map_err(|e| e.user_message())?;
    #[cfg(windows)]
    {
        crate::platform::open_path_in_explorer(&dir.to_string_lossy())
            .map_err(|e| e.user_message())?;
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<AppState>) -> Result<AppConfig, String> {
    let mut cfg = AppConfig::default();
    // 保留缓存亮度? 规范：重置所有设置
    cfg.validate_mut();
    save_config(&cfg).map_err(|e| e.user_message())?;
    *state.config.write() = cfg.clone();
    let _ = state.hotkeys.register_all(&app, &cfg.hotkeys);
    let _ = app.emit(events::EVT_SETTINGS_CHANGED, &cfg);
    Ok(cfg)
}

#[tauri::command]
pub fn report_ui_error(message: String, code: Option<String>) {
    let mut rate = UI_ERROR_RATE.lock();
    if rate.window_started.elapsed() >= Duration::from_secs(60) {
        if rate.dropped > 0 {
            tracing::warn!(dropped = rate.dropped, "前端错误上报已限流");
        }
        rate.window_started = Instant::now();
        rate.accepted = 0;
        rate.dropped = 0;
    }
    if rate.accepted >= UI_ERROR_LIMIT_PER_MINUTE {
        rate.dropped = rate.dropped.saturating_add(1);
        return;
    }
    rate.accepted += 1;
    drop(rate);

    let message: String = message.chars().take(UI_ERROR_MESSAGE_MAX_CHARS).collect();
    let code = code.map(|value| value.chars().take(UI_ERROR_CODE_MAX_CHARS).collect::<String>());
    tracing::warn!(message = %message, code = ?code, "前端错误");
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    crate::update::open_external_url(&url)
}

#[tauri::command]
pub fn check_for_updates(force: Option<bool>) -> Result<crate::update::UpdateCheckResult, String> {
    Ok(crate::update::check_for_updates(force.unwrap_or(false)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinks {
    pub github_url: String,
    pub releases_url: String,
    pub owner: String,
    pub repo: String,
}

#[tauri::command]
pub fn get_project_links() -> ProjectLinks {
    ProjectLinks {
        github_url: crate::update::GITHUB_URL.into(),
        releases_url: crate::update::GITHUB_RELEASES_URL.into(),
        owner: crate::update::GITHUB_OWNER.into(),
        repo: crate::update::GITHUB_REPO.into(),
    }
}

/// 点击外部关闭：由前端在 blur 时调用；Rust 也可挂全局
#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}
