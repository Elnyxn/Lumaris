//! 后端文案（托盘 / tooltip）— 与前端语言包键语义对齐

#[derive(Debug, Clone, Copy)]
pub struct TrayI18n {
    pub open: &'static str,
    pub increase: &'static str,
    pub decrease: &'static str,
    pub prev: &'static str,
    pub next: &'static str,
    pub sync: &'static str,
    pub refresh: &'static str,
    pub settings: &'static str,
    pub autostart: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
}

const ZH: TrayI18n = TrayI18n {
    open: "打开亮度控制",
    increase: "增加亮度",
    decrease: "降低亮度",
    prev: "上一台显示器",
    next: "下一台显示器",
    sync: "同步所有显示器",
    refresh: "刷新显示器",
    settings: "设置",
    autostart: "开机自启",
    quit: "退出",
    tooltip: "Lumaris — 点击打开 · 悬停滚轮调亮度",
};

const EN: TrayI18n = TrayI18n {
    open: "Open brightness control",
    increase: "Increase brightness",
    decrease: "Decrease brightness",
    prev: "Previous display",
    next: "Next display",
    sync: "Sync all displays",
    refresh: "Refresh displays",
    settings: "Settings",
    autostart: "Start with Windows",
    quit: "Quit",
    tooltip: "Lumaris — click to open · scroll to adjust",
};

pub fn tray_i18n(locale: &str) -> TrayI18n {
    let s = locale.trim().to_ascii_lowercase();
    if s == "en" || s.starts_with("en-") {
        EN
    } else {
        ZH
    }
}
