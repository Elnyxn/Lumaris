/** 简体中文 — 默认语言包 */
export const zhCN = {
  appName: "Lumaris",
  bootFailed: "启动失败",
  initFailed: "初始化失败，部分功能可能不可用",

  // flyout
  "flyout.noMonitor": "无显示器",
  "flyout.noMonitorDetected": "未检测到显示器",
  "flyout.monitors": "显示器",
  "flyout.monitorsCount": "{n} 台",
  "flyout.adjustBrightness": "调整亮度",
  "flyout.brightnessContrast": "亮度 / 对比度",
  "flyout.brightnessAria": "显示器亮度",
  "flyout.contrastAria": "显示器对比度",
  "flyout.settingsAria": "设置",
  "flyout.brightnessOf": "{name} 亮度",

  // settings sections
  "settings.title": "设置",
  "settings.back": "返回",
  "settings.general": "常规",
  "settings.brightness": "亮度",
  "settings.hotkeys": "快捷键",
  "settings.monitors": "显示器",
  "settings.ui": "界面",
  "settings.advanced": "高级",
  "settings.resetAll": "重置所有设置",
  "settings.version": "Lumaris v{v}",

  // general
  "settings.autostart": "开机自动启动",
  "settings.silentStartup": "启动时静默",
  "settings.delayedInit": "登录后延迟初始化",

  // brightness
  "settings.step": "亮度步长",
  "settings.stepHint": "快捷键与滚轮每次变化",
  "settings.stepCustom": "自定义",
  "settings.osd": "显示 OSD",
  "settings.osdDuration": "OSD 时长 (ms)",
  "settings.target": "快捷键控制目标",
  "settings.target.mouse": "鼠标所在显示器",
  "settings.target.lastUsed": "最近使用",
  "settings.target.primary": "主显示器",
  "settings.target.fixed": "固定显示器",
  "settings.target.allSync": "全部同步",
  "settings.syncAll": "同步所有显示器",
  "settings.rememberLast": "记忆最近设备",
  "settings.readOnStart": "启动时读取真实亮度",
  "settings.showContrast": "浮窗显示对比度（仅单屏）",

  // hotkeys
  "hotkey.increase": "增加亮度",
  "hotkey.decrease": "降低亮度",
  "hotkey.toggleFlyout": "显示/隐藏浮窗",
  "hotkey.prevMonitor": "上一台显示器",
  "hotkey.nextMonitor": "下一台显示器",
  "hotkey.syncIncrease": "同步增加亮度",
  "hotkey.syncDecrease": "同步降低亮度",
  "hotkey.reset": "恢复默认快捷键",
  "hotkey.unset": "未设置",
  "hotkey.press": "按下组合键…",
  "hotkey.needModifier": "需要修饰键",
  "hotkey.saved": "快捷键已保存",
  "hotkey.resetDone": "已恢复默认快捷键",
  "hotkey.recordAria": "录制快捷键",
  "hotkey.clear": "清除",
  "hotkey.clearAria": "清除快捷键",

  // monitors
  "monitors.empty": "未检测到显示器",
  "monitors.alias": "别名",
  "monitors.aliasPlaceholder": "可选",
  "monitors.sync": "参与同步",
  "monitors.fixed": "固定目标",
  "monitors.refresh": "刷新显示器",
  "monitors.refreshing": "正在刷新…",
  "monitors.refreshed": "已刷新 {n} 台",

  // ui
  "ui.appearance": "外观",
  "ui.theme.dark": "深色",
  "ui.theme.light": "浅色",
  "ui.language": "语言",
  "ui.lang.zhCN": "简体中文",
  "ui.lang.en": "English",
  "ui.animations": "启用动画",
  "ui.opacity": "界面透明度",

  // advanced
  "advanced.logLevel": "日志级别",
  "advanced.openLogs": "打开日志目录",
  "about.title": "关于",
  "about.github": "GitHub",
  "about.openRepo": "打开项目主页",
  "about.checkUpdate": "检查更新",
  "about.checking": "正在检查…",
  "about.upToDate": "已是最新版本 {v}",
  "about.updateAvailable": "发现新版本 {latest}（当前 {current}）",
  "about.openRelease": "打开发布页",
  "about.checkFailed": "检查失败：{err}",

  // toasts
  "toast.resetDone": "已重置设置",

  // status
  "status.available": "可用",
  "status.cached": "缓存",
  "status.temporarilyOffline": "暂时离线",
  "status.unsupported": "不支持",
  "status.readFailed": "读取失败",
  "status.writeFailed": "写入失败",
  "status.sleeping": "睡眠",
  "status.disconnected": "已断开",

  // control method
  "method.standard": "标准亮度 API",
  "method.vcp": "DDC/CI (VCP 0x10)",
  "method.wmi": "笔记本背光 (WMI)",
  "method.unsupported": "不支持",

  // tray (frontend mirror / docs)
  "tray.open": "打开亮度控制",
  "tray.increase": "增加亮度",
  "tray.decrease": "降低亮度",
  "tray.prev": "上一台显示器",
  "tray.next": "下一台显示器",
  "tray.sync": "同步所有显示器",
  "tray.refresh": "刷新显示器",
  "tray.settings": "设置",
  "tray.autostart": "开机自启",
  "tray.quit": "退出",
  "tray.tooltip": "Lumaris — 点击打开 · 悬停滚轮调亮度",
} as const;

export type MessageKey = keyof typeof zhCN;
export type Messages = Record<MessageKey, string>;
