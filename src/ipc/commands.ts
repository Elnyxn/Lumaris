import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppSnapshot,
  HotkeyConfig,
  MonitorInfo,
  OperationResultEvent,
} from "../state/types";

export async function frontendReady(): Promise<void> {
  await invoke("frontend_ready");
}

export async function getAppSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("get_app_snapshot");
}

export async function getMonitors(): Promise<MonitorInfo[]> {
  return invoke<MonitorInfo[]>("get_monitors");
}

export async function refreshMonitors(): Promise<void> {
  await invoke("refresh_monitors");
}

export async function setBrightness(
  monitorId: string,
  percent: number,
  finalWrite: boolean,
): Promise<void> {
  await invoke("set_brightness", {
    args: { monitorId, percent, finalWrite },
  });
}

export async function setContrast(
  monitorId: string,
  percent: number,
  finalWrite: boolean,
): Promise<void> {
  await invoke("set_contrast", {
    args: { monitorId, percent, finalWrite },
  });
}

export async function adjustBrightness(
  deltaSteps: number,
  finalWrite: boolean,
  opts?: { syncAll?: boolean; monitorId?: string },
): Promise<OperationResultEvent[]> {
  return invoke("adjust_brightness", {
    args: {
      deltaSteps,
      finalWrite,
      syncAll: opts?.syncAll ?? null,
      monitorId: opts?.monitorId ?? null,
    },
  });
}

export async function selectMonitor(monitorId: string): Promise<void> {
  await invoke("select_monitor", { monitorId });
}

export async function selectNextMonitor(): Promise<MonitorInfo | null> {
  return invoke("select_next_monitor");
}

export async function selectPrevMonitor(): Promise<MonitorInfo | null> {
  return invoke("select_prev_monitor");
}

export async function showFlyout(page?: string): Promise<void> {
  // 尺寸由后端固定，前端只传页面
  await invoke("show_flyout", {
    args: { height: null, page: page ?? "flyout" },
  });
}

export async function hideFlyout(): Promise<void> {
  await invoke("hide_flyout");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function updateConfig(
  patch: Record<string, unknown>,
): Promise<AppConfig> {
  return invoke("update_config", { patch });
}

export async function setMonitorAlias(
  monitorId: string,
  alias: string | null,
): Promise<void> {
  await invoke("set_monitor_alias", { monitorId, alias });
}

export async function setMonitorSyncInclude(
  monitorId: string,
  include: boolean,
): Promise<void> {
  await invoke("set_monitor_sync_include", { monitorId, include });
}

export async function setHotkey(
  action: string,
  accelerator: string | null,
): Promise<string | null> {
  return invoke("set_hotkey", {
    args: { action, accelerator },
  });
}

export async function resetHotkeys(): Promise<HotkeyConfig> {
  return invoke("reset_hotkeys");
}

export async function setAutostart(enabled: boolean): Promise<boolean> {
  return invoke("set_autostart", { enabled });
}

export async function getAutostart(): Promise<boolean> {
  return invoke("get_autostart");
}

export async function openLogsDir(): Promise<void> {
  await invoke("open_logs_dir");
}

export async function resetSettings(): Promise<AppConfig> {
  return invoke("reset_settings");
}

export async function reportUiError(
  message: string,
  code?: string,
): Promise<void> {
  await invoke("report_ui_error", { message, code: code ?? null });
}
