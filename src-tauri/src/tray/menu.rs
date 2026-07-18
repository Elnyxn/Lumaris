//! 系统托盘

use crate::state::AppState;
use std::thread;
use std::time::Duration;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, Wry,
};

pub fn setup_tray(app: &AppHandle<Wry>) -> tauri::Result<()> {
    let locale = {
        let state = app.state::<AppState>();
        let cfg = state.config.read();
        let loc = cfg.locale.clone();
        drop(cfg);
        loc
    };
    let i18n = crate::i18n::tray_i18n(&locale);

    let open = MenuItem::with_id(app, "open", i18n.open, true, None::<&str>)?;
    let increase = MenuItem::with_id(app, "increase", i18n.increase, true, None::<&str>)?;
    let decrease = MenuItem::with_id(app, "decrease", i18n.decrease, true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", i18n.prev, true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", i18n.next, true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", i18n.sync, true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", i18n.refresh, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", i18n.settings, true, None::<&str>)?;

    let autostart_on = {
        let state = app.state::<AppState>();
        let cfg = state.config.read();
        crate::startup::StartupManager::is_enabled(app) || cfg.autostart
    };
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        i18n.autostart,
        true,
        autostart_on,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", i18n.quit, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &sep,
            &increase,
            &decrease,
            &prev,
            &next,
            &sync,
            &refresh,
            &sep,
            &settings,
            &autostart,
            &sep,
            &quit,
        ],
    )?;

    let icon = load_tray_icon(app)?;

    // 已有托盘则只更新菜单/tooltip（语言切换）
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.set_tooltip(Some(i18n.tooltip));
        tracing::info!("系统托盘文案已更新");
        return Ok(());
    }

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip(i18n.tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            handle_menu(app, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    toggle_flyout_from_tray(app);
                }
                // 悬停期间滚轮可调亮度：同步图标物理矩形，供 WH_MOUSE_LL 点落检测
                // （远程桌面常丢 Leave，不能只靠 sticky bool）
                TrayIconEvent::Enter { rect, .. } | TrayIconEvent::Move { rect, .. } => {
                    crate::tray::tray_wheel::update_icon_rect_from_tauri(&rect);
                }
                TrayIconEvent::Leave { .. } => {
                    crate::tray::tray_wheel::set_tray_hover(false);
                }
                _ => {}
            }
        })
        .build(app)?;

    tracing::info!("系统托盘已就绪");
    Ok(())
}

/// 托盘单击 toggle：用逻辑 flyout_open，不依赖 is_visible（失焦会先藏窗）
pub fn toggle_flyout_from_tray(app: &AppHandle<Wry>) {
    let state = app.state::<AppState>();
    // 1) 抑制并取消排队中的失焦隐藏
    state.arm_suppress_focus_hide();

    let Some(win) = app.get_webview_window("main") else {
        state.clear_suppress_focus_hide();
        return;
    };

    // 2) 以语义状态为准（或仍可见）
    let should_hide = state.is_flyout_open() || win.is_visible().unwrap_or(false);

    if should_hide {
        let _ = win.hide();
        state.set_flyout_open(false);
        let _ = app.emit(crate::events::EVT_WINDOW_HIDDEN, ());
        tracing::debug!("托盘：隐藏浮窗");
    } else {
        let _ = crate::commands::show_flyout(
            app.clone(),
            app.state::<AppState>(),
            crate::commands::ShowFlyoutArgs {
                height: None,
                page: Some("flyout".into()),
            },
        );
        // 不再 emit ui-show-flyout，避免前端再走一遍 open/show 造成“二次显示”
        let _ = app.emit(crate::events::EVT_WINDOW_SHOWN, Some("flyout".to_string()));
        tracing::debug!("托盘：显示浮窗");
    }

    // 3) 延迟解除抑制（覆盖失焦事件到达窗口）
    let app2 = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(350));
        app2.state::<AppState>().clear_suppress_focus_hide();
    });
}

fn load_tray_icon(_app: &AppHandle<Wry>) -> tauri::Result<tauri::image::Image<'static>> {
    const PNG: &[u8] = include_bytes!("../../icons/32x32.png");
    tauri::image::Image::from_bytes(PNG).map_err(|e| {
        tracing::error!(error = %e, "托盘图标加载失败");
        tauri::Error::FailedToReceiveMessage
    })
}

fn handle_menu(app: &AppHandle<Wry>, id: &str) {
    match id {
        "open" => {
            toggle_flyout_from_tray(app);
        }
        "increase" => {
            let _ = app.emit(
                crate::events::EVT_HOTKEY_ACTION,
                crate::events::HotkeyActionEvent {
                    action: "increase".into(),
                    monitor_id: None,
                    final_write: true,
                    steps: 1,
                    by_points: false,
                },
            );
        }
        "decrease" => {
            let _ = app.emit(
                crate::events::EVT_HOTKEY_ACTION,
                crate::events::HotkeyActionEvent {
                    action: "decrease".into(),
                    monitor_id: None,
                    final_write: true,
                    steps: 1,
                    by_points: false,
                },
            );
        }
        "prev" => {
            let _ = app.emit(
                crate::events::EVT_HOTKEY_ACTION,
                crate::events::HotkeyActionEvent {
                    action: "prev_monitor".into(),
                    monitor_id: None,
                    final_write: true,
                    steps: 1,
                    by_points: false,
                },
            );
        }
        "next" => {
            let _ = app.emit(
                crate::events::EVT_HOTKEY_ACTION,
                crate::events::HotkeyActionEvent {
                    action: "next_monitor".into(),
                    monitor_id: None,
                    final_write: true,
                    steps: 1,
                    by_points: false,
                },
            );
        }
        "sync" => {
            let state = app.state::<AppState>();
            let mut cfg = state.config.write();
            cfg.sync_all = true;
            drop(cfg);
            let _ = app.emit(
                crate::events::EVT_HOTKEY_ACTION,
                crate::events::HotkeyActionEvent {
                    action: "sync_increase".into(),
                    monitor_id: None,
                    final_write: true,
                    steps: 1,
                    by_points: false,
                },
            );
        }
        "refresh" => {
            let state = app.state::<AppState>();
            state.monitors.force_refresh();
        }
        "settings" => {
            let _ = app.emit("tray-open-settings", ());
        }
        "autostart" => {
            let state = app.state::<AppState>();
            let currently = crate::startup::StartupManager::is_enabled(app);
            match crate::startup::StartupManager::set_enabled(app, !currently) {
                Ok(actual) => {
                    state.config.write().autostart = actual;
                    state.schedule_save();
                }
                Err(e) => {
                    tracing::error!(error = %e, "切换自启失败");
                    let _ = app.emit(
                        crate::events::EVT_ERROR,
                        crate::events::AppErrorEvent {
                            message: e.user_message(),
                            code: "autostart".into(),
                        },
                    );
                }
            }
        }
        "quit" => {
            let state = app.state::<AppState>();
            state.shutdown(app);
            app.exit(0);
        }
        _ => {}
    }
}

pub fn update_tooltip<R: Runtime>(app: &AppHandle<R>, text: &str) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(text));
    }
}
