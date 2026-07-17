//! 全局快捷键注册与冲突处理
//! 亮度相关：短按用设置步长；长按用时间加速度（百分点/秒），高频 tick 丝滑

use super::model::{is_system_reserved, normalize_accelerator};
use crate::error::{LumarisError, LumarisResult};
use crate::settings::HotkeyConfig;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 首次按下后多久进入连发
const REPEAT_INITIAL_MS: u64 = 160;
/// 连发 tick
const REPEAT_TICK_MS: u64 = 20;

pub struct HotkeyManager {
    registered: Mutex<HashMap<String, String>>,
    pub(crate) held: Mutex<Option<String>>,
    pub(crate) hold_gen: AtomicU64,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered: Mutex::new(HashMap::new()),
            held: Mutex::new(None),
            hold_gen: AtomicU64::new(0),
        }
    }

    pub fn registered_map(&self) -> HashMap<String, String> {
        self.registered.lock().clone()
    }

    pub fn unregister_all<R: Runtime>(&self, app: &AppHandle<R>) {
        self.stop_hold();
        let mut map = self.registered.lock();
        for (_action, accel) in map.drain() {
            if let Ok(shortcut) = accel.parse::<Shortcut>() {
                let _ = app.global_shortcut().unregister(shortcut);
            }
        }
    }

    pub fn register_all<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        cfg: &HotkeyConfig,
    ) -> Vec<(String, Result<(), String>)> {
        self.unregister_all(app);
        let mut results = Vec::new();
        for (action, accel) in cfg.iter_actions() {
            let Some(accel) = accel else {
                results.push((action.to_string(), Ok(())));
                continue;
            };
            match self.register_one(app, action, accel) {
                Ok(()) => results.push((action.to_string(), Ok(()))),
                Err(e) => {
                    tracing::warn!(action, error = %e, "快捷键注册失败");
                    results.push((action.to_string(), Err(e.user_message())));
                }
            }
        }
        results
    }

    pub fn register_one<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        action: &str,
        accel: &str,
    ) -> LumarisResult<()> {
        let normalized = normalize_accelerator(accel)?;
        if is_system_reserved(&normalized) {
            return Err(LumarisError::hotkey("该快捷键为系统保留组合"));
        }
        let shortcut: Shortcut = normalized
            .parse()
            .map_err(|e| LumarisError::hotkey(format!("快捷键语法无效: {e}")))?;

        let action_owned = action.to_string();
        let repeatable = is_repeatable_action(action);

        app.global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                match event.state {
                    ShortcutState::Pressed => {
                        // 短按：1 个设置步长；mode=steps
                        emit_hotkey(app, &action_owned, !repeatable, 1, false);
                        if repeatable {
                            start_hold_repeat(app, &action_owned);
                        }
                    }
                    ShortcutState::Released => {
                        if repeatable {
                            stop_hold_repeat(app, &action_owned);
                            emit_hotkey(app, &action_owned, true, 0, false);
                        }
                    }
                }
            })
            .map_err(|e| {
                LumarisError::hotkey(format!(
                    "该快捷键可能已被系统或其他程序占用。({e})"
                ))
            })?;

        self.registered
            .lock()
            .insert(action.to_string(), normalized);
        Ok(())
    }

    pub fn update_action<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        action: &str,
        new_accel: Option<&str>,
    ) -> LumarisResult<Option<String>> {
        let old = self.registered.lock().get(action).cloned();

        if let Some(ref o) = old {
            if let Ok(sc) = o.parse::<Shortcut>() {
                let _ = app.global_shortcut().unregister(sc);
            }
            self.registered.lock().remove(action);
        }

        let Some(accel) = new_accel.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };

        match self.register_one(app, action, accel) {
            Ok(()) => Ok(self.registered.lock().get(action).cloned()),
            Err(e) => {
                if let Some(ref o) = old {
                    if let Err(re) = self.register_one(app, action, o) {
                        tracing::error!(error = %re, "恢复旧快捷键失败");
                    }
                }
                Err(e)
            }
        }
    }

    fn stop_hold(&self) {
        self.hold_gen.fetch_add(1, Ordering::SeqCst);
        *self.held.lock() = None;
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

fn is_repeatable_action(action: &str) -> bool {
    matches!(
        action,
        "increase" | "decrease" | "sync_increase" | "sync_decrease"
    )
}

/// `by_points`: true 时 steps 表示绝对百分点；false 时表示设置步长的倍数
fn emit_hotkey<R: Runtime>(
    app: &AppHandle<R>,
    action: &str,
    final_write: bool,
    steps: i32,
    by_points: bool,
) {
    let _ = app.emit(
        crate::events::EVT_HOTKEY_ACTION,
        crate::events::HotkeyActionEvent {
            action: action.to_string(),
            monitor_id: None,
            final_write,
            steps,
            by_points,
        },
    );
}

/// 按住时长 → 亮度变化速率（百分点/秒），二次加速，约 1.6s 顶到峰值
fn hold_rate_pps(elapsed_ms: u64) -> f64 {
    let t = (elapsed_ms as f64 / 1600.0).clamp(0.0, 1.0);
    let ease = t * t; // ease-in
    // 起速 40%/s，峰值约 100%/s
    40.0 + 60.0 * ease
}

fn start_hold_repeat<R: Runtime>(app: &AppHandle<R>, action: &str) {
    let state = app.state::<crate::state::AppState>();
    let gen = state.hotkeys.hold_gen.fetch_add(1, Ordering::SeqCst) + 1;
    *state.hotkeys.held.lock() = Some(action.to_string());

    let app = app.clone();
    let action = action.to_string();
    thread::Builder::new()
        .name("lumaris-hotkey-repeat".into())
        .spawn(move || {
            let hold_start = Instant::now();
            thread::sleep(Duration::from_millis(REPEAT_INITIAL_MS));
            // 用浮点累加 residual，避免整数截断导致卡顿
            let mut residual = 0.0f64;
            loop {
                let st = app.state::<crate::state::AppState>();
                let current_gen = st.hotkeys.hold_gen.load(Ordering::SeqCst);
                let held = st.hotkeys.held.lock().clone();
                if current_gen != gen || held.as_deref() != Some(action.as_str()) {
                    break;
                }
                drop(held);
                drop(st);

                let elapsed = hold_start.elapsed().as_millis() as u64;
                let rate = hold_rate_pps(elapsed);
                residual += rate * (REPEAT_TICK_MS as f64) / 1000.0;
                let points = residual.floor() as i32;
                if points >= 1 {
                    residual -= points as f64;
                    // 单 tick 上限，防跳变过大
                    let pts = points.min(4);
                    emit_hotkey(&app, &action, false, pts, true);
                }
                thread::sleep(Duration::from_millis(REPEAT_TICK_MS));
            }
        })
        .ok();
}

fn stop_hold_repeat<R: Runtime>(app: &AppHandle<R>, action: &str) {
    let state = app.state::<crate::state::AppState>();
    let mut held = state.hotkeys.held.lock();
    if held.as_deref() == Some(action) {
        *held = None;
        state.hotkeys.hold_gen.fetch_add(1, Ordering::SeqCst);
    }
}
