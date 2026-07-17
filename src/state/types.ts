import { t } from "../i18n";

export type ControlMethod =
  | "standardBrightnessApi"
  | "vcpCode10"
  | "wmiAcpi"
  | "unsupported";

export type MonitorStatus =
  | "available"
  | "cached"
  | "temporarilyOffline"
  | "unsupported"
  | "readFailed"
  | "writeFailed"
  | "sleeping"
  | "disconnected";

export type TargetMode =
  | "mouseMonitor"
  | "lastUsed"
  | "primary"
  | "fixed"
  | "allSync";

export type LayoutMode = "single" | "list";

export type LogLevelSetting = "info" | "debug" | "warn" | "error";

export interface WorkArea {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface MonitorInfo {
  id: string;
  displayName: string;
  userAlias: string | null;
  description: string;
  currentBrightness: number;
  cachedBrightness: number;
  minBrightness: number;
  maxBrightness: number;
  currentContrast: number;
  cachedContrast: number;
  minContrast: number;
  maxContrast: number;
  contrastControllable: boolean;
  controlMethod: ControlMethod;
  status: MonitorStatus;
  isPrimary: boolean;
  isExternal: boolean;
  isSelected: boolean;
  isControllable: boolean;
  workArea: WorkArea;
  dpi: number;
}

export interface HotkeyConfig {
  increase: string | null;
  decrease: string | null;
  toggleFlyout: string | null;
  prevMonitor: string | null;
  nextMonitor: string | null;
  syncIncrease: string | null;
  syncDecrease: string | null;
}

export interface OsdSettings {
  enabled: boolean;
  autoHideMs: number;
}

export type ThemeMode = "dark" | "light";

export interface UiSettings {
  animations: boolean;
  opacity: number;
  /** dark=深色（默认），light=浅色 */
  theme?: ThemeMode;
}

export interface AppConfig {
  schemaVersion: number;
  hotkeys: HotkeyConfig;
  stepPercent: number;
  customStepPercent: number;
  autostart: boolean;
  silentStartup: boolean;
  delayedMonitorInit: boolean;
  delayedInitMs: number;
  osd: OsdSettings;
  ui: UiSettings;
  targetMode: TargetMode;
  lastMonitorId: string | null;
  fixedMonitorId: string | null;
  layoutMode: LayoutMode;
  syncAll: boolean;
  rememberLastMonitor: boolean;
  readBrightnessOnStart: boolean;
  /** 首屏是否显示对比度滑块 */
  showContrast: boolean;
  /** UI 语言：zh-CN | en */
  locale?: string;
  monitorAliases: Record<string, string>;
  monitorSyncInclude: Record<string, boolean>;
  cachedBrightness: Record<string, number>;
  cachedContrast: Record<string, number>;
  logLevel: LogLevelSetting;
}

export interface AppSnapshot {
  config: AppConfig;
  monitors: MonitorInfo[];
  autostartEnabled: boolean;
  startupMode: boolean;
  version: string;
}

export type PageId = "flyout" | "settings";

export interface BrightnessChangedEvent {
  monitorId: string;
  brightness: number;
  cached: boolean;
  status: string;
}

export interface OperationResultEvent {
  monitorId: string;
  success: boolean;
  brightness: number | null;
  error: string | null;
  kind: string;
}

export interface AppErrorEvent {
  message: string;
  code: string;
}

export const HOTKEY_ACTIONS: { id: keyof HotkeyConfig }[] = [
  { id: "increase" },
  { id: "decrease" },
  { id: "toggleFlyout" },
  { id: "prevMonitor" },
  { id: "nextMonitor" },
  { id: "syncIncrease" },
  { id: "syncDecrease" },
];

export function monitorLabel(m: MonitorInfo): string {
  return m.userAlias?.trim() || m.displayName;
}

export function statusLabel(s: MonitorStatus | string): string {
  const key = String(s);
  const map: Record<string, () => string> = {
    available: () => t("status.available"),
    Available: () => t("status.available"),
    cached: () => t("status.cached"),
    Cached: () => t("status.cached"),
    temporarilyOffline: () => t("status.temporarilyOffline"),
    TemporarilyOffline: () => t("status.temporarilyOffline"),
    unsupported: () => t("status.unsupported"),
    Unsupported: () => t("status.unsupported"),
    readFailed: () => t("status.readFailed"),
    ReadFailed: () => t("status.readFailed"),
    writeFailed: () => t("status.writeFailed"),
    WriteFailed: () => t("status.writeFailed"),
    sleeping: () => t("status.sleeping"),
    Sleeping: () => t("status.sleeping"),
    disconnected: () => t("status.disconnected"),
    Disconnected: () => t("status.disconnected"),
  };
  return map[key]?.() ?? key;
}

export function controlMethodLabel(m: ControlMethod | string): string {
  const key = String(m);
  const map: Record<string, () => string> = {
    standardBrightnessApi: () => t("method.standard"),
    StandardBrightnessApi: () => t("method.standard"),
    vcpCode10: () => t("method.vcp"),
    VcpCode10: () => t("method.vcp"),
    wmiAcpi: () => t("method.wmi"),
    WmiAcpi: () => t("method.wmi"),
    unsupported: () => t("method.unsupported"),
    Unsupported: () => t("method.unsupported"),
  };
  return map[key]?.() ?? key;
}

/** Rust action snake_case ↔ 前端 camelCase */
export function hotkeyActionToRust(id: keyof HotkeyConfig): string {
  const map: Record<keyof HotkeyConfig, string> = {
    increase: "increase",
    decrease: "decrease",
    toggleFlyout: "toggle_flyout",
    prevMonitor: "prev_monitor",
    nextMonitor: "next_monitor",
    syncIncrease: "sync_increase",
    syncDecrease: "sync_decrease",
  };
  return map[id];
}
