//! 显示器管理器：状态、目标选择、与 worker 协作

use super::device::{MonitorInfo, MonitorStatus};
use super::worker::{MonitorCommand, MonitorWorker, WorkerEvent};
use crate::error::{LumarisError, LumarisResult};
use crate::settings::{AppConfig, TargetMode};
use crate::utils::clamp_percent;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct MonitorManager {
    monitors: RwLock<Vec<MonitorInfo>>,
    selected_id: RwLock<Option<String>>,
    worker: RwLock<Option<MonitorWorker>>,
    request_seq: AtomicU64,
    sleeping: RwLock<bool>,
    last_refresh: RwLock<Option<Instant>>,
    debounce_ms: u64,
    /// 每个显示器最近一次亮度写入请求 id（用于丢弃过期回写，防 UI 回滚）
    brightness_req: RwLock<HashMap<String, u64>>,
}

impl MonitorManager {
    pub fn new() -> Self {
        Self {
            monitors: RwLock::new(Vec::new()),
            selected_id: RwLock::new(None),
            worker: RwLock::new(None),
            request_seq: AtomicU64::new(1),
            sleeping: RwLock::new(false),
            last_refresh: RwLock::new(None),
            debounce_ms: 800,
            brightness_req: RwLock::new(HashMap::new()),
        }
    }

    pub fn start_worker_with_tx(&self, event_tx: Sender<WorkerEvent>) {
        let worker = MonitorWorker::start(event_tx);
        *self.worker.write() = Some(worker);
    }

    fn next_req(&self) -> u64 {
        self.request_seq.fetch_add(1, Ordering::Relaxed)
    }

    pub fn request_refresh(&self) {
        if *self.sleeping.read() {
            return;
        }
        // 防抖
        {
            let mut last = self.last_refresh.write();
            if let Some(t) = *last {
                if t.elapsed() < Duration::from_millis(self.debounce_ms) {
                    return;
                }
            }
            *last = Some(Instant::now());
        }
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::RefreshMonitors {
                request_id: self.next_req(),
            });
        }
    }

    pub fn force_refresh(&self) {
        *self.last_refresh.write() = None;
        *self.sleeping.write() = false;
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::RefreshMonitors {
                request_id: self.next_req(),
            });
        } else {
            tracing::warn!("force_refresh：worker 尚未就绪");
        }
    }

    pub fn set_brightness(&self, id: &str, percent: u32, final_write: bool) {
        if *self.sleeping.read() {
            return;
        }
        let p = percent.min(100);
        {
            let mut list = self.monitors.write();
            if let Some(m) = list.iter_mut().find(|m| m.id == id) {
                m.cached_brightness = p;
                m.current_brightness = p;
            }
        }
        let request_id = self.next_req();
        self.brightness_req.write().insert(id.to_string(), request_id);
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::SetBrightness {
                id: id.to_string(),
                percent: p,
                final_write,
                request_id,
            });
        }
    }

    /// 是否仍是该显示器最新一次亮度写入（用于丢弃过期回写）
    pub fn is_latest_brightness_req(&self, id: &str, request_id: u64) -> bool {
        self.brightness_req
            .read()
            .get(id)
            .map(|r| *r == request_id)
            .unwrap_or(true)
    }

    pub fn set_contrast(&self, id: &str, percent: u32, final_write: bool) {
        if *self.sleeping.read() {
            return;
        }
        {
            let mut list = self.monitors.write();
            if let Some(m) = list.iter_mut().find(|m| m.id == id) {
                let p = percent.min(100);
                m.cached_contrast = p;
                m.current_contrast = p;
            }
        }
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::SetContrast {
                id: id.to_string(),
                percent: percent.min(100),
                final_write,
                request_id: self.next_req(),
            });
        }
    }

    pub fn read_brightness(&self, id: &str) {
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::ReadBrightness {
                id: id.to_string(),
                request_id: self.next_req(),
            });
        }
    }

    pub fn read_contrast(&self, id: &str) {
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::ReadContrast {
                id: id.to_string(),
                request_id: self.next_req(),
            });
        }
    }

    pub fn pause(&self) {
        *self.sleeping.write() = true;
        {
            let mut list = self.monitors.write();
            for m in list.iter_mut() {
                if m.status == MonitorStatus::Available {
                    m.status = MonitorStatus::Sleeping;
                }
            }
        }
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::Pause);
        }
    }

    pub fn resume(&self) {
        *self.sleeping.write() = false;
        if let Some(w) = self.worker.read().as_ref() {
            w.send(MonitorCommand::Resume);
        }
        // 延迟刷新由 app 层调度
    }

    pub fn shutdown(&self) {
        if let Some(mut w) = self.worker.write().take() {
            w.shutdown();
        }
    }

    pub fn apply_monitors(&self, mut list: Vec<MonitorInfo>, cfg: &AppConfig) {
        for m in list.iter_mut() {
            if let Some(alias) = cfg.monitor_aliases.get(&m.id) {
                m.user_alias = Some(alias.clone());
            }
            if let Some(&cached) = cfg.cached_brightness.get(&m.id) {
                if m.status != MonitorStatus::Available {
                    m.cached_brightness = cached;
                    m.current_brightness = cached;
                    if m.status == MonitorStatus::Unsupported {
                        // keep
                    } else {
                        m.status = MonitorStatus::Cached;
                    }
                }
            }
            if let Some(&cached) = cfg.cached_contrast.get(&m.id) {
                if !m.contrast_controllable {
                    m.cached_contrast = cached;
                    m.current_contrast = cached;
                }
            }
        }

        // 恢复选择
        let prev = self.selected_id.read().clone();
        let selected = prev
            .filter(|id| list.iter().any(|m| &m.id == id))
            .or_else(|| list.iter().find(|m| m.is_primary).map(|m| m.id.clone()))
            .or_else(|| list.first().map(|m| m.id.clone()));

        for m in list.iter_mut() {
            m.is_selected = selected.as_ref() == Some(&m.id);
        }

        *self.selected_id.write() = selected;
        *self.monitors.write() = list;
    }

    pub fn list(&self) -> Vec<MonitorInfo> {
        self.monitors.read().clone()
    }

    pub fn selected(&self) -> Option<MonitorInfo> {
        let id = self.selected_id.read().clone()?;
        self.monitors.read().iter().find(|m| m.id == id).cloned()
    }

    pub fn select(&self, id: &str) -> LumarisResult<()> {
        let mut list = self.monitors.write();
        if !list.iter().any(|m| m.id == id) {
            return Err(LumarisError::monitor("显示器不存在"));
        }
        for m in list.iter_mut() {
            m.is_selected = m.id == id;
        }
        *self.selected_id.write() = Some(id.to_string());
        Ok(())
    }

    pub fn select_next(&self) -> Option<MonitorInfo> {
        let mut list = self.monitors.write();
        if list.is_empty() {
            return None;
        }
        let cur = list.iter().position(|m| m.is_selected).unwrap_or(0);
        let next = (cur + 1) % list.len();
        for (i, m) in list.iter_mut().enumerate() {
            m.is_selected = i == next;
        }
        let id = list[next].id.clone();
        *self.selected_id.write() = Some(id);
        Some(list[next].clone())
    }

    pub fn select_prev(&self) -> Option<MonitorInfo> {
        let mut list = self.monitors.write();
        if list.is_empty() {
            return None;
        }
        let cur = list.iter().position(|m| m.is_selected).unwrap_or(0);
        let prev = if cur == 0 { list.len() - 1 } else { cur - 1 };
        for (i, m) in list.iter_mut().enumerate() {
            m.is_selected = i == prev;
        }
        let id = list[prev].id.clone();
        *self.selected_id.write() = Some(id);
        Some(list[prev].clone())
    }

    pub fn update_brightness_state(
        &self,
        id: &str,
        percent: u32,
        success: bool,
        write: bool,
    ) {
        self.update_brightness_state_req(id, percent, success, write, None);
    }

    /// `request_id` 若有：仅当仍是最新写入时才覆盖显示值，避免滚轮/长按回弹
    pub fn update_brightness_state_req(
        &self,
        id: &str,
        percent: u32,
        success: bool,
        write: bool,
        request_id: Option<u64>,
    ) {
        let latest = request_id
            .map(|rid| self.is_latest_brightness_req(id, rid))
            .unwrap_or(true);
        let mut list = self.monitors.write();
        if let Some(m) = list.iter_mut().find(|m| m.id == id) {
            if success {
                if latest {
                    m.current_brightness = percent;
                    m.cached_brightness = percent;
                }
                // 过期成功回写：不改 cached，避免 UI 回滚
                m.status = MonitorStatus::Available;
            } else if write {
                // 过期失败也不要改显示；仅最新失败标红
                if latest {
                    m.status = MonitorStatus::WriteFailed;
                }
            } else {
                m.status = MonitorStatus::ReadFailed;
            }
        }
    }

    /// 解析快捷键/操作目标
    pub fn resolve_targets(&self, cfg: &AppConfig) -> Vec<String> {
        if cfg.sync_all || cfg.target_mode == TargetMode::AllSync {
            return self
                .monitors
                .read()
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
            TargetMode::MouseMonitor => {
                let list = self.list();
                crate::platform::get_cursor_monitor_id(&list)
                    .or_else(|| self.selected_id.read().clone())
                    .or_else(|| cfg.last_monitor_id.clone())
                    .or_else(|| {
                        list.iter()
                            .find(|m| m.is_primary)
                            .map(|m| m.id.clone())
                    })
                    .or_else(|| {
                        list.iter()
                            .find(|m| m.is_controllable)
                            .map(|m| m.id.clone())
                    })
            }
            TargetMode::LastUsed => cfg
                .last_monitor_id
                .clone()
                .or_else(|| self.selected_id.read().clone()),
            TargetMode::Primary => self
                .monitors
                .read()
                .iter()
                .find(|m| m.is_primary)
                .map(|m| m.id.clone()),
            TargetMode::Fixed => cfg
                .fixed_monitor_id
                .clone()
                .or_else(|| self.selected_id.read().clone()),
            TargetMode::AllSync => None,
        };

        id.into_iter().collect()
    }

    /// `delta`：有符号的「步数」（±1、±2…），最终变化 = delta × effective_step
    pub fn adjust_brightness(
        &self,
        cfg: &AppConfig,
        delta: i32,
        final_write: bool,
        ids: Option<Vec<String>>,
    ) -> Vec<(String, u32)> {
        let step = cfg.effective_step() as i32;
        let points = delta.saturating_mul(step);
        self.adjust_brightness_points(cfg, points, final_write, ids)
    }

    /// 按绝对百分点调节（长按/滚轮丝滑用，不绑死设置步长）
    pub fn adjust_brightness_points(
        &self,
        cfg: &AppConfig,
        delta_points: i32,
        final_write: bool,
        ids: Option<Vec<String>>,
    ) -> Vec<(String, u32)> {
        if delta_points == 0 {
            return Vec::new();
        }
        let targets = ids.unwrap_or_else(|| self.resolve_targets(cfg));
        let mut results = Vec::new();
        for id in targets {
            let mut list = self.monitors.write();
            if let Some(m) = list.iter_mut().find(|m| m.id == id) {
                if !m.is_controllable && m.status == MonitorStatus::Unsupported {
                    continue;
                }
                let next = clamp_percent(m.cached_brightness as i32 + delta_points);
                m.cached_brightness = next;
                m.current_brightness = next;
                results.push((id.clone(), next));
            }
            drop(list);
            if let Some((_, pct)) = results.iter().rev().find(|(i, _)| i == &id) {
                self.set_brightness(&id, *pct, final_write);
            }
        }
        results
    }
}

impl Default for MonitorManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 供 AppState 共享
pub type SharedMonitorManager = Arc<MonitorManager>;
