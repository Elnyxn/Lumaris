//! 全局快捷键注册与冲突处理
//! 亮度相关：短按用设置步长；长按用时间加速度（百分点/秒），高频 tick 丝滑

use super::model::{is_system_reserved, normalize_accelerator};
use crate::error::{LumarisError, LumarisResult};
use crate::settings::HotkeyConfig;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
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
    event_tx: OnceCell<Sender<HotkeyInput>>,
    event_epoch: Arc<AtomicU64>,
    pub(crate) held: Mutex<Option<String>>,
    pub(crate) hold_gen: AtomicU64,
}

struct HotkeyInput {
    action: String,
    state: ShortcutState,
    epoch: u64,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered: Mutex::new(HashMap::new()),
            event_tx: OnceCell::new(),
            event_epoch: Arc::new(AtomicU64::new(0)),
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
        // 注销期间已派发的回调也必须失效，不能在注销后重新启动连发。
        self.stop_hold();
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
        let tx = self.event_sender(app)?;
        let epoch = self.event_epoch.clone();

        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                // 插件持有快捷键表锁；这里不能发业务事件或同步等待 UI。
                // 按下/松开统一排队，避免每次 spawn 导致顺序颠倒。
                let _ = tx.send(HotkeyInput {
                    action: action_owned.clone(),
                    state: event.state,
                    epoch: epoch.load(Ordering::SeqCst),
                });
            })
            .map_err(|e| {
                LumarisError::hotkey(format!("该快捷键可能已被系统或其他程序占用。({e})"))
            })?;

        self.registered
            .lock()
            .insert(action.to_string(), normalized);
        Ok(())
    }

    fn event_sender<R: Runtime>(&self, app: &AppHandle<R>) -> LumarisResult<Sender<HotkeyInput>> {
        self.event_tx
            .get_or_try_init(|| {
                let (tx, rx) = mpsc::channel::<HotkeyInput>();
                let app = app.clone();
                let epoch = self.event_epoch.clone();
                thread::Builder::new()
                    .name("lumaris-hotkeys".into())
                    .spawn(move || {
                        while let Ok(input) = rx.recv() {
                            if input.epoch != epoch.load(Ordering::SeqCst) {
                                continue;
                            }
                            let repeatable = is_repeatable_action(&input.action);
                            match input.state {
                                ShortcutState::Pressed => {
                                    emit_hotkey(&app, &input.action, !repeatable, 1, false);
                                    if repeatable {
                                        start_hold_repeat(&app, &input.action, input.epoch);
                                    }
                                }
                                ShortcutState::Released => {
                                    if repeatable {
                                        app.state::<crate::state::AppState>()
                                            .hotkeys
                                            .end_hold(&input.action, input.epoch);
                                        emit_hotkey(&app, &input.action, true, 0, false);
                                    }
                                }
                            }
                        }
                    })?;
                Ok::<_, LumarisError>(tx)
            })
            .cloned()
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
            self.stop_hold();
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
        let mut held = self.held.lock();
        self.event_epoch.fetch_add(1, Ordering::SeqCst);
        self.hold_gen.fetch_add(1, Ordering::SeqCst);
        *held = None;
    }

    fn begin_hold(&self, action: &str, epoch: u64) -> Option<u64> {
        let mut held = self.held.lock();
        if epoch != self.event_epoch.load(Ordering::SeqCst) {
            return None;
        }
        let gen = self.hold_gen.fetch_add(1, Ordering::SeqCst) + 1;
        *held = Some(action.to_string());
        Some(gen)
    }

    fn end_hold(&self, action: &str, epoch: u64) {
        let mut held = self.held.lock();
        if epoch == self.event_epoch.load(Ordering::SeqCst) && held.as_deref() == Some(action) {
            *held = None;
            self.hold_gen.fetch_add(1, Ordering::SeqCst);
        }
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
    // ease-in：起速 40%/s，峰值约 100%/s
    let ease = t * t;
    40.0 + 60.0 * ease
}

fn start_hold_repeat<R: Runtime>(app: &AppHandle<R>, action: &str, epoch: u64) {
    let state = app.state::<crate::state::AppState>();
    let Some(gen) = state.hotkeys.begin_hold(action, epoch) else {
        return;
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_press_cannot_restart_repeat_after_unregistration() {
        let manager = HotkeyManager::new();
        let epoch = manager.event_epoch.load(Ordering::SeqCst);
        assert!(manager.begin_hold("increase", epoch).is_some());
        manager.stop_hold();
        assert!(manager.begin_hold("increase", epoch).is_none());
        assert!(manager.held.lock().is_none());
    }

    #[test]
    fn stale_release_cannot_stop_a_new_registration_hold() {
        let manager = HotkeyManager::new();
        let old = manager.event_epoch.load(Ordering::SeqCst);
        manager.stop_hold();
        let current = manager.event_epoch.load(Ordering::SeqCst);
        assert!(manager.begin_hold("increase", current).is_some());
        manager.end_hold("increase", old);
        assert_eq!(manager.held.lock().as_deref(), Some("increase"));
        manager.end_hold("increase", current);
        assert!(manager.held.lock().is_none());
    }
}
