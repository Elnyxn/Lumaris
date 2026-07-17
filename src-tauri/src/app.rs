//! 应用装配与生命周期

use crate::commands;
use crate::events::{self, BrightnessChangedEvent, OperationResultEvent};
use crate::logging;
use crate::monitor::WorkerEvent;
use crate::settings::load_config;
use crate::state::AppState;
use crate::tray;
use crate::window::{apply_backdrop, prepare_transparent_window, AppSystemEvent, SystemMessageHub};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    let startup_mode = std::env::args().any(|a| a == "--startup");

    let config = load_config().unwrap_or_default();
    let log_guard = match logging::init_logging(config.log_level) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("日志初始化失败: {e}");
            // 最小 fallback：继续运行
            logging::init_logging(crate::settings::LogLevelSetting::Info)
                .expect("日志二次初始化失败")
        }
    };

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        startup_mode,
        "Lumaris 启动"
    );

    let state = AppState::new(config, log_guard, startup_mode);
    // 供 setup 闭包使用
    let show_on_start = !startup_mode;

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("检测到第二实例，显示已有浮窗");
            let _ = app.emit("show-flyout", ());
            let _ = app.emit("ui-show-flyout", serde_json::json!({ "page": "flyout" }));
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--startup"]),
        ))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::frontend_ready,
            commands::get_app_snapshot,
            commands::get_monitors,
            commands::refresh_monitors,
            commands::set_brightness,
            commands::set_contrast,
            commands::adjust_brightness,
            commands::select_monitor,
            commands::select_next_monitor,
            commands::select_prev_monitor,
            commands::show_flyout,
            commands::hide_flyout,
            commands::resize_flyout,
            commands::get_config,
            commands::update_config,
            commands::set_monitor_alias,
            commands::set_monitor_sync_include,
            commands::set_hotkey,
            commands::reset_hotkeys,
            commands::set_autostart,
            commands::get_autostart,
            commands::open_logs_dir,
            commands::reset_settings,
            commands::report_ui_error,
            commands::ping,
        ])
        .setup(move |app| {
            tracing::info!("进入 setup");
            let handle = app.handle().clone();
            let state = handle.state::<AppState>();

            // 托盘（失败不崩溃）
            match tray::setup_tray(&handle) {
                Ok(()) => tracing::info!("托盘初始化成功"),
                Err(e) => {
                    tracing::error!(error = %e, "托盘初始化失败");
                }
            }

            // 窗口初始
            if let Some(win) = handle.get_webview_window("main") {
                prepare_transparent_window(&win);
                let _ = win.hide();
                let win_clone = win.clone();
                let app_for_focus = handle.clone();
                win.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let st = app_for_focus.state::<AppState>();
                            st.set_flyout_open(false);
                            let _ = win_clone.hide();
                            let _ = app_for_focus.emit(crate::events::EVT_WINDOW_HIDDEN, ());
                        }
                        // 点击窗外：延迟隐藏，给托盘点击留出窗口（避免先失焦再 toggle 误判）
                        tauri::WindowEvent::Focused(false) => {
                            let st = app_for_focus.state::<AppState>();
                            if st.suppress_focus_hide.load(std::sync::atomic::Ordering::SeqCst) {
                                return;
                            }
                            if !st.is_flyout_open() && !win_clone.is_visible().unwrap_or(false) {
                                return;
                            }
                            let gen = st.schedule_focus_hide_gen();
                            let app2 = app_for_focus.clone();
                            let win2 = win_clone.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(180));
                                let st = app2.state::<AppState>();
                                if st.focus_hide_gen() != gen {
                                    return; // 已取消（托盘点击等）
                                }
                                if st.suppress_focus_hide.load(std::sync::atomic::Ordering::SeqCst)
                                {
                                    return;
                                }
                                if !st.is_flyout_open() {
                                    return;
                                }
                                st.set_flyout_open(false);
                                let _ = win2.hide();
                                let _ = app2.emit(crate::events::EVT_WINDOW_HIDDEN, ());
                            });
                        }
                        tauri::WindowEvent::Focused(true) => {
                            // 获得焦点时取消排队隐藏
                            let st = app_for_focus.state::<AppState>();
                            let _ = st.schedule_focus_hide_gen();
                        }
                        _ => {}
                    }
                });
            }

            // 快捷键
            {
                let cfg = state.config.read().clone();
                let results = state.hotkeys.register_all(&handle, &cfg.hotkeys);
                for (action, res) in results {
                    if let Err(e) = res {
                        tracing::warn!(action, error = %e, "快捷键未注册");
                    }
                }
            }

            // 托盘悬停滚轮调亮度
            {
                let (wtx, wrx) = mpsc::channel::<i32>();
                if let Err(e) = crate::tray::tray_wheel::install(wtx) {
                    tracing::warn!(error = %e, "托盘滚轮钩子安装失败");
                } else {
                    let h = handle.clone();
                    thread::Builder::new()
                        .name("lumaris-tray-wheel".into())
                        .spawn(move || {
                            while let Ok(notches) = wrx.recv() {
                                handle_tray_wheel(&h, notches);
                            }
                        })
                        .ok();
                }
            }

            // DDC worker 事件通道
            let (wtx, wrx) = mpsc::channel::<WorkerEvent>();
            state.monitors.start_worker_with_tx(wtx);

            // 系统消息
            let sys_hub = SystemMessageHub::create();

            // 启动时枚举显示器
            let delayed = {
                let cfg = state.config.read();
                cfg.delayed_monitor_init && startup_mode
            };
            let delay_ms = state.config.read().delayed_init_ms;
            if delayed {
                let h = handle.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(delay_ms));
                    let st = h.state::<AppState>();
                    st.monitors.force_refresh();
                });
            } else {
                state.monitors.force_refresh();
            }

            // worker 事件转发线程
            {
                let h = handle.clone();
                thread::Builder::new()
                    .name("lumaris-worker-fwd".into())
                    .spawn(move || {
                        while let Ok(ev) = wrx.recv() {
                            handle_worker_event(&h, ev);
                        }
                    })
                    .ok();
            }

            // 系统事件 + 延迟保存轮询（低频）
            {
                let h = handle.clone();
                thread::Builder::new()
                    .name("lumaris-idle".into())
                    .spawn(move || {
                        loop {
                            thread::sleep(Duration::from_millis(400));
                            let st = h.state::<AppState>();
                            st.flush_save_if_due();

                            while let Some(sev) = sys_hub.try_recv() {
                                handle_system_event(&h, sev);
                            }
                        }
                    })
                    .ok();
            }

            // 前端事件：托盘/快捷键显示浮窗
            {
                let h = handle.clone();
                handle.listen("show-flyout", move |_| {
                    let _ = h.emit("ui-show-flyout", serde_json::json!({ "page": "flyout" }));
                });
            }
            // tray-toggle 已在 tray 层直接处理，不再转发前端二次 toggle
            {
                let h = handle.clone();
                handle.listen("tray-open-settings", move |_| {
                    let _ = h.emit("ui-show-flyout", serde_json::json!({ "page": "settings" }));
                });
            }

            // 快捷键动作（含按住连发）
            {
                let h = handle.clone();
                handle.listen(events::EVT_HOTKEY_ACTION, move |event| {
                    let payload = event.payload();
                    if let Ok(action_ev) =
                        serde_json::from_str::<events::HotkeyActionEvent>(payload)
                    {
                        handle_hotkey_action(
                            &h,
                            &action_ev.action,
                            action_ev.final_write,
                            action_ev.steps,
                            action_ev.by_points,
                        );
                    }
                });
            }

            // 交互启动：等前端 + 至少一次显示器枚举后再 show，避免首次左上角
            if show_on_start {
                let h = handle.clone();
                thread::spawn(move || {
                    let mut ready = false;
                    let mut has_mon = false;
                    for _ in 0..80 {
                        thread::sleep(Duration::from_millis(50));
                        let st = h.state::<AppState>();
                        ready = st.is_frontend_ready();
                        has_mon = !st.monitors.list().is_empty();
                        if ready && has_mon {
                            break;
                        }
                    }
                    tracing::info!(ready, has_mon, "首次显示浮窗");
                    // 先定位再显示，禁止裸 show 落到 (0,0)
                    if let Some(win) = h.get_webview_window("main") {
                        let st = h.state::<AppState>();
                        // 始终主屏
                        let mons = st.monitors.list();
                        let mon_count = mons.len();
                        let (work, dpi) = mons
                            .into_iter()
                            .find(|m| m.is_primary)
                            .map(|m| (m.work_area, m.dpi))
                            .unwrap_or_else(|| {
                                (
                                    crate::platform::primary_work_area(),
                                    crate::platform::primary_dpi(),
                                )
                            });
                        let show_c = st.config.read().show_contrast && mon_count <= 1;
                        let page = st.current_page();
                        let _ = crate::window::position_flyout(
                            &win,
                            &work,
                            Some(page.as_str()),
                            dpi,
                            show_c,
                            mon_count,
                        );
                        let _ = win.set_ignore_cursor_events(false);
                        let _ = win.show();
                        let _ = win.set_focus();
                        h.state::<AppState>().set_flyout_open(true);
                    }
                    let _ = h.emit(
                        "ui-show-flyout",
                        serde_json::json!({ "page": "flyout", "quiet": true }),
                    );
                });
            }

            tracing::info!("setup 完成");
            Ok(())
        });

    tracing::info!("正在构建 Tauri 上下文…");
    let app = match builder.build(tauri::generate_context!()) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, "Tauri 构建失败");
            // 写入易发现的错误文件
            if let Ok(dir) = crate::settings::app_data_dir() {
                let _ = std::fs::write(
                    dir.join("last_error.txt"),
                    format!("Tauri 构建失败: {e}"),
                );
            }
            return;
        }
    };
    tracing::info!("进入事件循环");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = &event {
            let _ = api;
        }
        if let tauri::RunEvent::Exit = event {
            let state = app.state::<AppState>();
            state.shutdown(app);
        }
    });
}

fn handle_worker_event(app: &tauri::AppHandle, ev: WorkerEvent) {
    let state = app.state::<AppState>();
    match ev {
        WorkerEvent::MonitorsRefreshed {
            monitors,
            success,
            error,
            ..
        } => {
            if success {
                let cfg = state.config.read().clone();
                state.monitors.apply_monitors(monitors, &cfg);
                // 启动读取真实亮度
                if cfg.read_brightness_on_start {
                    for m in state.monitors.list() {
                        if m.is_controllable {
                            state.monitors.read_brightness(&m.id);
                        }
                        if m.contrast_controllable {
                            state.monitors.read_contrast(&m.id);
                        }
                    }
                }
                let list = state.monitors.list();
                let _ = app.emit(events::EVT_MONITORS_CHANGED, &list);
                update_tray_tooltip(app);
                // 枚举完成后静默校正位置（不抢焦点、不二次闪窗）
                if let Some(win) = app.get_webview_window("main") {
                    if win.is_visible().unwrap_or(false) {
                        let (work, dpi) = list
                            .iter()
                            .find(|m| m.is_primary)
                            .map(|m| (m.work_area.clone(), m.dpi))
                            .or_else(|| {
                                list.first().map(|m| (m.work_area.clone(), m.dpi))
                            })
                            .unwrap_or_else(|| {
                                (
                                    crate::platform::primary_work_area(),
                                    crate::platform::primary_dpi(),
                                )
                            });
                        let mon_count = list.len();
                        let show_c = state.config.read().show_contrast && mon_count <= 1;
                        let page = state.current_page();
                        let _ = crate::window::position_flyout(
                            &win,
                            &work,
                            Some(page.as_str()),
                            dpi,
                            show_c,
                            mon_count,
                        );
                    }
                }
            } else if let Some(err) = error {
                tracing::error!(error = %err, "刷新显示器失败");
                let _ = app.emit(
                    events::EVT_ERROR,
                    events::AppErrorEvent {
                        message: err,
                        code: "monitor".into(),
                    },
                );
            }
        }
        WorkerEvent::BrightnessSet {
            id,
            percent,
            success,
            error,
            final_write,
            request_id,
            ..
        } => {
            let is_latest = state.monitors.is_latest_brightness_req(&id, request_id);
            state.monitors.update_brightness_state_req(
                &id,
                percent,
                success,
                true,
                Some(request_id),
            );
            // 仅最新写入才推 UI，杜绝滚轮/长按「回弹」
            if is_latest {
                let shown = state
                    .monitors
                    .list()
                    .into_iter()
                    .find(|m| m.id == id)
                    .map(|m| m.cached_brightness)
                    .unwrap_or(percent);
                let _ = app.emit(
                    events::EVT_BRIGHTNESS_CHANGED,
                    BrightnessChangedEvent {
                        monitor_id: id.clone(),
                        brightness: shown,
                        cached: !success,
                        status: if success {
                            "Available".into()
                        } else {
                            "WriteFailed".into()
                        },
                    },
                );
            }
            if !success && is_latest {
                let _ = app.emit(
                    events::EVT_OPERATION_RESULT,
                    OperationResultEvent {
                        monitor_id: id.clone(),
                        success,
                        brightness: Some(percent),
                        error,
                        kind: if final_write {
                            "set_final".into()
                        } else {
                            "set".into()
                        },
                    },
                );
            }
            if success && final_write && is_latest {
                state.schedule_save();
            }
            if is_latest {
                update_tray_tooltip(app);
            }
        }
        WorkerEvent::ContrastSet {
            id,
            percent,
            success,
            error,
            final_write,
            ..
        } => {
            {
                let mut list = state.monitors.list();
                for m in list.iter_mut() {
                    if m.id == id {
                        if success {
                            m.current_contrast = percent;
                            m.cached_contrast = percent;
                        }
                    }
                }
                let cfg = state.config.read().clone();
                state.monitors.apply_monitors(list, &cfg);
            }
            if final_write {
                let _ = app.emit(
                    events::EVT_CONTRAST_CHANGED,
                    events::ContrastChangedEvent {
                        monitor_id: id,
                        contrast: percent,
                        cached: !success,
                        status: if success {
                            "Available".into()
                        } else {
                            "WriteFailed".into()
                        },
                    },
                );
                if success {
                    state.schedule_save();
                }
            }
            let _ = error;
        }
        WorkerEvent::ContrastRead {
            id,
            percent,
            min,
            max,
            success,
            ..
        } => {
            if success {
                if let Some(p) = percent {
                    let mut list = state.monitors.list();
                    for m in list.iter_mut() {
                        if m.id == id {
                            m.current_contrast = p;
                            m.cached_contrast = p;
                            m.min_contrast = min;
                            m.max_contrast = max;
                            m.contrast_controllable = true;
                        }
                    }
                    let cfg = state.config.read().clone();
                    state.monitors.apply_monitors(list, &cfg);
                    let _ = app.emit(
                        events::EVT_CONTRAST_CHANGED,
                        events::ContrastChangedEvent {
                            monitor_id: id,
                            contrast: p,
                            cached: false,
                            status: "Available".into(),
                        },
                    );
                    let _ = app.emit(events::EVT_MONITORS_CHANGED, &state.monitors.list());
                }
            }
        }
        WorkerEvent::BrightnessRead {
            id,
            percent,
            min,
            max,
            success,
            error,
            method,
            ..
        } => {
            if success {
                if let Some(p) = percent {
                    let mut list = state.monitors.list();
                    for m in list.iter_mut() {
                        if m.id == id {
                            m.current_brightness = p;
                            m.cached_brightness = p;
                            m.min_brightness = min;
                            m.max_brightness = max;
                            m.status = crate::monitor::MonitorStatus::Available;
                            if let Some(meth) = method {
                                m.control_method = meth;
                            }
                        }
                    }
                    let cfg = state.config.read().clone();
                    state.monitors.apply_monitors(list, &cfg);
                    let _ = app.emit(
                        events::EVT_BRIGHTNESS_CHANGED,
                        BrightnessChangedEvent {
                            monitor_id: id,
                            brightness: p,
                            cached: false,
                            status: "Available".into(),
                        },
                    );
                }
            } else {
                state
                    .monitors
                    .update_brightness_state(&id, 0, false, false);
                if let Some(err) = error {
                    tracing::warn!(id = %logging::short_id(&id), error = %err, "读亮度失败");
                }
            }
            let _ = app.emit(events::EVT_MONITORS_CHANGED, &state.monitors.list());
        }
        WorkerEvent::ShutdownDone => {
            tracing::info!("Worker 已关闭");
        }
    }
}

fn handle_system_event(app: &tauri::AppHandle, ev: AppSystemEvent) {
    let state = app.state::<AppState>();
    match ev {
        AppSystemEvent::DisplayChange | AppSystemEvent::DeviceChange => {
            tracing::info!(?ev, "显示器拓扑变化");
            state.monitors.request_refresh();
        }
        AppSystemEvent::PowerSuspend => {
            tracing::info!("系统休眠");
            state.monitors.pause();
        }
        AppSystemEvent::PowerResume => {
            tracing::info!("系统唤醒，延迟刷新显示器");
            state.monitors.resume();
            let h = app.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                let st = h.state::<AppState>();
                st.monitors.force_refresh();
            });
        }
        AppSystemEvent::DpiChanged => {
            tracing::info!("DPI 变化");
            state.monitors.request_refresh();
        }
        AppSystemEvent::ThemeChanged | AppSystemEvent::SettingChange => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = apply_backdrop(&win);
            }
        }
        AppSystemEvent::TaskbarCreated => {
            tracing::info!("Explorer 重启，托盘应已由 Tauri 恢复");
            // 重新确保托盘存在
            let _ = tray::setup_tray(app);
        }
    }
}

fn handle_hotkey_action(
    app: &tauri::AppHandle,
    action: &str,
    final_write: bool,
    steps: i32,
    by_points: bool,
) {
    let state = app.state::<AppState>();
    let steps = steps.clamp(-12, 12);
    match action {
        "increase" | "decrease" | "sync_increase" | "sync_decrease" => {
            let sign: i32 = if action.contains("decrease") { -1 } else { 1 };
            let sync = action.starts_with("sync_");
            let mut results = Vec::new();
            if steps != 0 {
                let mut cfg = state.config.read().clone();
                if sync {
                    cfg.sync_all = true;
                }
                results = if by_points {
                    state.monitors.adjust_brightness_points(
                        &cfg,
                        sign * steps.abs(),
                        final_write,
                        None,
                    )
                } else {
                    state.monitors.adjust_brightness(
                        &cfg,
                        sign * steps.abs(),
                        final_write,
                        None,
                    )
                };
            } else if final_write {
                let mut cfg = state.config.read().clone();
                if sync {
                    cfg.sync_all = true;
                }
                for id in state.monitors.resolve_targets(&cfg) {
                    if let Some(m) = state.monitors.list().iter().find(|m| m.id == id) {
                        let pct = m.cached_brightness;
                        state.monitors.set_brightness(&id, pct, true);
                        results.push((id, pct));
                    }
                }
            }
            // 立即推 UI（不等硬件）；附带最新亮度，前端可先写入 store 再首绘，避免滑块跳
            for (id, pct) in &results {
                let _ = app.emit(
                    events::EVT_BRIGHTNESS_CHANGED,
                    BrightnessChangedEvent {
                        monitor_id: id.clone(),
                        brightness: *pct,
                        cached: true,
                        status: "Available".into(),
                    },
                );
            }
            // 只通知前端 quiet OSD：由前端先画对亮度再 show，后端不抢先 show/focus
            let win_visible = app
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            if win_visible {
                state.set_flyout_open(true);
            }
            let snap: Vec<serde_json::Value> = results
                .iter()
                .map(|(id, pct)| serde_json::json!({ "id": id, "brightness": pct }))
                .collect();
            let _ = app.emit(
                "ui-show-flyout",
                serde_json::json!({
                    "page": "flyout",
                    "quiet": true,
                    "brightness": snap,
                }),
            );
        }
        "prev_monitor" => {
            let _ = state.monitors.select_prev();
            let _ = app.emit(events::EVT_MONITORS_CHANGED, &state.monitors.list());
            let _ = app.emit(
                "ui-show-flyout",
                serde_json::json!({ "page": "flyout", "quiet": true }),
            );
        }
        "next_monitor" => {
            let _ = state.monitors.select_next();
            let _ = app.emit(events::EVT_MONITORS_CHANGED, &state.monitors.list());
            let _ = app.emit(
                "ui-show-flyout",
                serde_json::json!({ "page": "flyout", "quiet": true }),
            );
        }
        "toggle_flyout" => {
            let _ = app.emit("ui-toggle-flyout", ());
        }
        _ => {}
    }
    update_tray_tooltip(app);
}

/// 托盘滚轮：正数增亮、负数减亮；中间 final=false，靠防抖线程写 final
fn handle_tray_wheel(app: &tauri::AppHandle, notches: i32) {
    if notches == 0 {
        return;
    }
    // 每格 2 个百分点，手感细腻且不跟设置步长死绑
    let points = notches.clamp(-4, 4) * 2;
    let action = if points > 0 { "increase" } else { "decrease" };
    handle_hotkey_action(app, action, false, points.abs(), true);
    schedule_tray_wheel_final(app);
}

/// 滚轮停 120ms 后写 final，合并硬件写
fn schedule_tray_wheel_final(app: &tauri::AppHandle) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static GEN: AtomicU64 = AtomicU64::new(0);
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(120));
        if GEN.load(Ordering::SeqCst) != gen {
            return;
        }
        let state = app.state::<AppState>();
        let cfg = state.config.read().clone();
        for id in state.monitors.resolve_targets(&cfg) {
            if let Some(m) = state.monitors.list().iter().find(|m| m.id == id) {
                state
                    .monitors
                    .set_brightness(&id, m.cached_brightness, true);
            }
        }
        state.schedule_save();
    });
}

fn update_tray_tooltip(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if let Some(m) = state.monitors.selected() {
        let text = format!("Lumaris\n{}\n亮度 {}%", m.label(), m.cached_brightness);
        tray::menu::update_tooltip(app, &text);
    } else {
        tray::menu::update_tooltip(app, "Lumaris");
    }
}
