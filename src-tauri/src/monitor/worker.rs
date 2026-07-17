//! 固定后台工作线程：串行 DDC 操作 + 待写入合并

use super::ddc;
use super::device::ControlMethod;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum MonitorCommand {
    SetBrightness {
        id: String,
        percent: u32,
        final_write: bool,
        request_id: u64,
    },
    SetContrast {
        id: String,
        percent: u32,
        final_write: bool,
        request_id: u64,
    },
    ReadBrightness {
        id: String,
        request_id: u64,
    },
    ReadContrast {
        id: String,
        request_id: u64,
    },
    RefreshMonitors {
        request_id: u64,
    },
    Pause,
    Resume,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum WorkerEvent {
    BrightnessSet {
        id: String,
        percent: u32,
        method: ControlMethod,
        success: bool,
        error: Option<String>,
        request_id: u64,
        final_write: bool,
    },
    ContrastSet {
        id: String,
        percent: u32,
        success: bool,
        error: Option<String>,
        request_id: u64,
        final_write: bool,
    },
    BrightnessRead {
        id: String,
        percent: Option<u32>,
        min: u32,
        max: u32,
        method: Option<ControlMethod>,
        success: bool,
        error: Option<String>,
        request_id: u64,
    },
    ContrastRead {
        id: String,
        percent: Option<u32>,
        min: u32,
        max: u32,
        success: bool,
        error: Option<String>,
        request_id: u64,
    },
    MonitorsRefreshed {
        monitors: Vec<super::device::MonitorInfo>,
        success: bool,
        error: Option<String>,
        request_id: u64,
    },
    ShutdownDone,
}

struct PendingWrite {
    percent: u32,
    final_write: bool,
    request_id: u64,
}

pub struct MonitorWorker {
    cmd_tx: Sender<MonitorCommand>,
    handle: Option<JoinHandle<()>>,
}

impl MonitorWorker {
    pub fn start(event_tx: Sender<WorkerEvent>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<MonitorCommand>();
        let handle = thread::Builder::new()
            .name("lumaris-ddc".into())
            .spawn(move || worker_loop(cmd_rx, event_tx))
            .expect("无法创建 DDC 工作线程");
        Self {
            cmd_tx,
            handle: Some(handle),
        }
    }

    pub fn sender(&self) -> Sender<MonitorCommand> {
        self.cmd_tx.clone()
    }

    pub fn send(&self, cmd: MonitorCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn shutdown(&mut self) {
        let _ = self.cmd_tx.send(MonitorCommand::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for MonitorWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_loop(rx: Receiver<MonitorCommand>, event_tx: Sender<WorkerEvent>) {
    let pending: Arc<Mutex<HashMap<String, PendingWrite>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_contrast: Arc<Mutex<HashMap<String, PendingWrite>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let mut paused = false;
    let mut cool_until: HashMap<String, Instant> = HashMap::new();
    let cooldown = Duration::from_secs(3);
    let max_retries = 2u32;

    loop {
        let cmd = match rx.recv() {
            Ok(c) => c,
            Err(_) => break,
        };

        let mut batch = vec![cmd];
        while let Ok(more) = rx.try_recv() {
            batch.push(more);
        }

        let mut refresh: Option<u64> = None;
        let mut reads: Vec<(String, u64)> = Vec::new();
        let mut contrast_reads: Vec<(String, u64)> = Vec::new();
        let mut shutdown = false;

        for cmd in batch {
            match cmd {
                MonitorCommand::Shutdown => {
                    shutdown = true;
                }
                MonitorCommand::Pause => {
                    paused = true;
                    pending.lock().clear();
                    pending_contrast.lock().clear();
                    tracing::info!("DDC 工作线程已暂停");
                }
                MonitorCommand::Resume => {
                    paused = false;
                    tracing::info!("DDC 工作线程已恢复");
                }
                MonitorCommand::RefreshMonitors { request_id } => {
                    refresh = Some(request_id);
                }
                MonitorCommand::ReadBrightness { id, request_id } => {
                    reads.push((id, request_id));
                }
                MonitorCommand::ReadContrast { id, request_id } => {
                    contrast_reads.push((id, request_id));
                }
                MonitorCommand::SetBrightness {
                    id,
                    percent,
                    final_write,
                    request_id,
                } => {
                    let mut map = pending.lock();
                    let entry = map.entry(id).or_insert(PendingWrite {
                        percent,
                        final_write,
                        request_id,
                    });
                    entry.percent = percent.min(100);
                    entry.request_id = request_id;
                    if final_write {
                        entry.final_write = true;
                    }
                }
                MonitorCommand::SetContrast {
                    id,
                    percent,
                    final_write,
                    request_id,
                } => {
                    let mut map = pending_contrast.lock();
                    let entry = map.entry(id).or_insert(PendingWrite {
                        percent,
                        final_write,
                        request_id,
                    });
                    entry.percent = percent.min(100);
                    entry.request_id = request_id;
                    if final_write {
                        entry.final_write = true;
                    }
                }
            }
        }

        if shutdown {
            let finals: Vec<(String, PendingWrite)> = pending
                .lock()
                .drain()
                .filter(|(_, p)| p.final_write)
                .collect();
            for (id, p) in finals {
                let _ = try_write(&id, p.percent, max_retries, &mut cool_until, cooldown);
            }
            let c_finals: Vec<(String, PendingWrite)> = pending_contrast
                .lock()
                .drain()
                .filter(|(_, p)| p.final_write)
                .collect();
            for (id, p) in c_finals {
                let _ = try_write_contrast(&id, p.percent, max_retries, &mut cool_until, cooldown);
            }
            let _ = event_tx.send(WorkerEvent::ShutdownDone);
            break;
        }

        if paused {
            continue;
        }

        if let Some(request_id) = refresh {
            pending.lock().retain(|_, p| p.final_write);
            pending_contrast.lock().retain(|_, p| p.final_write);
            match ddc::enumerate() {
                Ok(monitors) => {
                    let _ = event_tx.send(WorkerEvent::MonitorsRefreshed {
                        monitors,
                        success: true,
                        error: None,
                        request_id,
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(WorkerEvent::MonitorsRefreshed {
                        monitors: vec![],
                        success: false,
                        error: Some(e.user_message()),
                        request_id,
                    });
                }
            }
        }

        // 亮度写入
        let to_write: Vec<(String, PendingWrite)> = {
            let mut map = pending.lock();
            let keys: Vec<String> = map.keys().cloned().collect();
            let mut out = Vec::new();
            for k in keys {
                if let Some(p) = map.remove(&k) {
                    out.push((k, p));
                }
            }
            out.sort_by(|a, b| b.1.final_write.cmp(&a.1.final_write));
            out
        };

        for (id, p) in to_write {
            if let Some(until) = cool_until.get(&id) {
                if Instant::now() < *until {
                    let _ = event_tx.send(WorkerEvent::BrightnessSet {
                        id: id.clone(),
                        percent: p.percent,
                        method: ControlMethod::Unsupported,
                        success: false,
                        error: Some("设备冷却中".into()),
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                    continue;
                }
            }

            match try_write(&id, p.percent, max_retries, &mut cool_until, cooldown) {
                Ok((pct, method)) => {
                    let _ = event_tx.send(WorkerEvent::BrightnessSet {
                        id: id.clone(),
                        percent: pct,
                        method,
                        success: true,
                        error: None,
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                }
                Err(err) => {
                    cool_until.insert(id.clone(), Instant::now() + cooldown);
                    let _ = event_tx.send(WorkerEvent::BrightnessSet {
                        id: id.clone(),
                        percent: p.percent,
                        method: ControlMethod::Unsupported,
                        success: false,
                        error: Some(err),
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                }
            }
        }

        // 对比度写入
        let to_c_write: Vec<(String, PendingWrite)> = {
            let mut map = pending_contrast.lock();
            let keys: Vec<String> = map.keys().cloned().collect();
            let mut out = Vec::new();
            for k in keys {
                if let Some(p) = map.remove(&k) {
                    out.push((k, p));
                }
            }
            out.sort_by(|a, b| b.1.final_write.cmp(&a.1.final_write));
            out
        };

        for (id, p) in to_c_write {
            if let Some(until) = cool_until.get(&id) {
                if Instant::now() < *until {
                    let _ = event_tx.send(WorkerEvent::ContrastSet {
                        id: id.clone(),
                        percent: p.percent,
                        success: false,
                        error: Some("设备冷却中".into()),
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                    continue;
                }
            }
            match try_write_contrast(&id, p.percent, max_retries, &mut cool_until, cooldown) {
                Ok(pct) => {
                    let _ = event_tx.send(WorkerEvent::ContrastSet {
                        id: id.clone(),
                        percent: pct,
                        success: true,
                        error: None,
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                }
                Err(err) => {
                    cool_until.insert(id.clone(), Instant::now() + cooldown);
                    let _ = event_tx.send(WorkerEvent::ContrastSet {
                        id: id.clone(),
                        percent: p.percent,
                        success: false,
                        error: Some(err),
                        request_id: p.request_id,
                        final_write: p.final_write,
                    });
                }
            }
        }

        for (id, request_id) in reads {
            if let Some(until) = cool_until.get(&id) {
                if Instant::now() < *until {
                    let _ = event_tx.send(WorkerEvent::BrightnessRead {
                        id,
                        percent: None,
                        min: 0,
                        max: 100,
                        method: None,
                        success: false,
                        error: Some("设备冷却中".into()),
                        request_id,
                    });
                    continue;
                }
            }
            match ddc::read_brightness(&id) {
                Ok((pct, min, max, method)) => {
                    let _ = event_tx.send(WorkerEvent::BrightnessRead {
                        id,
                        percent: Some(pct),
                        min,
                        max,
                        method: Some(method),
                        success: true,
                        error: None,
                        request_id,
                    });
                }
                Err(e) => {
                    cool_until.insert(id.clone(), Instant::now() + cooldown);
                    let _ = event_tx.send(WorkerEvent::BrightnessRead {
                        id,
                        percent: None,
                        min: 0,
                        max: 100,
                        method: None,
                        success: false,
                        error: Some(e.user_message()),
                        request_id,
                    });
                }
            }
        }

        for (id, request_id) in contrast_reads {
            match ddc::read_contrast(&id) {
                Ok((pct, min, max)) => {
                    let _ = event_tx.send(WorkerEvent::ContrastRead {
                        id,
                        percent: Some(pct),
                        min,
                        max,
                        success: true,
                        error: None,
                        request_id,
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(WorkerEvent::ContrastRead {
                        id,
                        percent: None,
                        min: 0,
                        max: 100,
                        success: false,
                        error: Some(e.user_message()),
                        request_id,
                    });
                }
            }
        }
    }

    tracing::info!("DDC 工作线程已退出");
}

fn try_write(
    id: &str,
    percent: u32,
    max_retries: u32,
    cool_until: &mut HashMap<String, Instant>,
    cooldown: Duration,
) -> Result<(u32, ControlMethod), String> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match ddc::write_brightness(id, percent) {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = e.user_message();
                tracing::warn!(
                    id = %crate::logging::short_id(id),
                    attempt,
                    error = %last_err,
                    "写入亮度失败"
                );
                if attempt < max_retries {
                    thread::sleep(Duration::from_millis(80 * (attempt as u64 + 1)));
                }
            }
        }
    }
    cool_until.insert(id.to_string(), Instant::now() + cooldown);
    Err(last_err)
}

fn try_write_contrast(
    id: &str,
    percent: u32,
    max_retries: u32,
    cool_until: &mut HashMap<String, Instant>,
    cooldown: Duration,
) -> Result<u32, String> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match ddc::write_contrast(id, percent) {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = e.user_message();
                tracing::warn!(
                    id = %crate::logging::short_id(id),
                    attempt,
                    error = %last_err,
                    "写入对比度失败"
                );
                if attempt < max_retries {
                    thread::sleep(Duration::from_millis(80 * (attempt as u64 + 1)));
                }
            }
        }
    }
    cool_until.insert(id.to_string(), Instant::now() + cooldown);
    Err(last_err)
}

/// 合并逻辑单测（无硬件）
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_overwrite() {
        let mut map: HashMap<String, PendingWrite> = HashMap::new();
        map.insert(
            "a".into(),
            PendingWrite {
                percent: 10,
                final_write: false,
                request_id: 1,
            },
        );
        if let Some(p) = map.get_mut("a") {
            p.percent = 40;
            p.request_id = 2;
            p.final_write = true;
        }
        let p = map.get("a").unwrap();
        assert_eq!(p.percent, 40);
        assert!(p.final_write);
        assert_eq!(p.request_id, 2);
    }
}
