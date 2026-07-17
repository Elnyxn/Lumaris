//! 全局应用状态

use crate::hotkey::HotkeyManager;
use crate::logging::LogGuard;
use crate::monitor::MonitorManager;
use crate::settings::{save_config, AppConfig};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Runtime};

pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub monitors: Arc<MonitorManager>,
    pub hotkeys: Arc<HotkeyManager>,
    pub frontend_ready: AtomicBool,
    pub startup_mode: AtomicBool,
    /// 用户语义上的浮窗是否应显示（不依赖 is_visible，避免托盘/失焦竞态）
    pub flyout_open: AtomicBool,
    /// 当前页面：flyout | settings（重定位时保持正确窗口高度）
    pub current_page: RwLock<String>,
    /// 托盘点击期间抑制失焦隐藏
    pub suppress_focus_hide: AtomicBool,
    /// 递增以取消已调度的失焦隐藏
    pub focus_hide_gen: AtomicU64,
    pub save_deadline: RwLock<Option<Instant>>,
    pub _log_guard: RwLock<Option<LogGuard>>,
}

impl AppState {
    pub fn new(config: AppConfig, log_guard: LogGuard, startup_mode: bool) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            monitors: Arc::new(MonitorManager::new()),
            hotkeys: Arc::new(HotkeyManager::new()),
            frontend_ready: AtomicBool::new(false),
            startup_mode: AtomicBool::new(startup_mode),
            flyout_open: AtomicBool::new(false),
            current_page: RwLock::new("flyout".into()),
            suppress_focus_hide: AtomicBool::new(false),
            focus_hide_gen: AtomicU64::new(0),
            save_deadline: RwLock::new(None),
            _log_guard: RwLock::new(Some(log_guard)),
        }
    }

    pub fn set_current_page(&self, page: &str) {
        *self.current_page.write() = page.to_string();
    }

    pub fn current_page(&self) -> String {
        self.current_page.read().clone()
    }

    pub fn set_flyout_open(&self, open: bool) {
        self.flyout_open.store(open, Ordering::SeqCst);
    }

    pub fn is_flyout_open(&self) -> bool {
        self.flyout_open.load(Ordering::SeqCst)
    }

    pub fn arm_suppress_focus_hide(&self) {
        self.suppress_focus_hide.store(true, Ordering::SeqCst);
        // 取消已排队的失焦隐藏
        self.focus_hide_gen.fetch_add(1, Ordering::SeqCst);
    }

    pub fn clear_suppress_focus_hide(&self) {
        self.suppress_focus_hide.store(false, Ordering::SeqCst);
    }

    pub fn schedule_focus_hide_gen(&self) -> u64 {
        self.focus_hide_gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn focus_hide_gen(&self) -> u64 {
        self.focus_hide_gen.load(Ordering::SeqCst)
    }

    /// 延迟保存：合并写盘
    pub fn schedule_save(&self) {
        *self.save_deadline.write() = Some(Instant::now() + Duration::from_millis(800));
    }

    pub fn flush_save_if_due(&self) {
        let due = {
            let guard = self.save_deadline.read();
            match *guard {
                Some(t) if Instant::now() >= t => true,
                _ => false,
            }
        };
        if due {
            self.save_now();
        }
    }

    pub fn save_now(&self) {
        *self.save_deadline.write() = None;
        let cfg = self.config.read().clone();
        // 同步缓存亮度
        let mut cfg = cfg;
        for m in self.monitors.list() {
            cfg.cached_brightness
                .insert(m.id.clone(), m.cached_brightness);
            cfg.cached_contrast
                .insert(m.id.clone(), m.cached_contrast);
        }
        if let Err(e) = save_config(&cfg) {
            tracing::error!(error = %e, "保存配置失败");
        } else {
            *self.config.write() = cfg;
            tracing::debug!("配置已保存");
        }
    }

    pub fn shutdown<R: Runtime>(&self, app: &AppHandle<R>) {
        tracing::info!("应用正在退出…");
        self.hotkeys.unregister_all(app);
        self.monitors.shutdown();
        self.save_now();
        #[cfg(windows)]
        {
            crate::platform::clear_physical_cache();
        }
    }

    pub fn mark_frontend_ready(&self) {
        self.frontend_ready.store(true, Ordering::SeqCst);
    }

    pub fn is_frontend_ready(&self) -> bool {
        self.frontend_ready.load(Ordering::SeqCst)
    }

    pub fn is_startup_mode(&self) -> bool {
        self.startup_mode.load(Ordering::SeqCst)
    }
}

pub fn get_state<R: Runtime>(app: &AppHandle<R>) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}
