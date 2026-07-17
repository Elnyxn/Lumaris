//! 日志：文件轮转 + 控制台（Debug）

use crate::error::LumarisResult;
use crate::settings::{logs_dir, LogLevelSetting};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// 保持 guard 存活，否则日志缓冲可能丢失
pub struct LogGuard {
    _file_guard: WorkerGuard,
}

const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 14;

pub fn init_logging(level: LogLevelSetting) -> LumarisResult<LogGuard> {
    let dir = logs_dir()?;
    maintain_log_files(&dir);

    let file_appender = tracing_appender::rolling::daily(&dir, "lumaris.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("lumaris={},info", level.as_filter())));

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(false);

    #[cfg(debug_assertions)]
    {
        let stdout_layer = fmt::layer().with_target(true).with_ansi(true);
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stdout_layer)
            .try_init();
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .try_init();
    }

    tracing::info!(level = level.as_filter(), "日志系统已初始化");
    Ok(LogGuard {
        _file_guard: guard,
    })
}

fn maintain_log_files(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<(PathBuf, SystemTime)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("lumaris.log") && !name.starts_with("lumaris.") {
                return None;
            }
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if meta.len() > MAX_LOG_FILE_BYTES && !name.contains(".oversize-") {
                let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
                let rotated = dir.join(format!("{name}.oversize-{stamp}"));
                if fs::rename(&path, &rotated).is_ok() {
                    return Some((rotated, modified));
                }
            }
            Some((path, modified))
        })
        .collect();

    logs.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    for (path, _) in logs.into_iter().skip(MAX_LOG_FILES) {
        let _ = fs::remove_file(path);
    }
}

/// 缩短显示器 ID 用于日志
pub fn short_id(id: &str) -> String {
    if id.len() <= 12 {
        id.to_string()
    } else {
        format!("{}…{}", &id[..6], &id[id.len() - 4..])
    }
}
