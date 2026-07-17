import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppConfig,
  AppErrorEvent,
  AppSnapshot,
  BrightnessChangedEvent,
  MonitorInfo,
  OperationResultEvent,
} from "../state/types";

export async function onMonitorsChanged(
  cb: (m: MonitorInfo[]) => void,
): Promise<UnlistenFn> {
  return listen<MonitorInfo[]>("monitors-changed", (e) => cb(e.payload));
}

export async function onBrightnessChanged(
  cb: (e: BrightnessChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<BrightnessChangedEvent>("brightness-changed", (ev) =>
    cb(ev.payload),
  );
}

export interface ContrastChangedEvent {
  monitorId: string;
  contrast: number;
  cached: boolean;
  status: string;
}

export async function onContrastChanged(
  cb: (e: ContrastChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ContrastChangedEvent>("contrast-changed", (ev) =>
    cb(ev.payload),
  );
}

export async function onOperationResult(
  cb: (e: OperationResultEvent) => void,
): Promise<UnlistenFn> {
  return listen<OperationResultEvent>("operation-result", (ev) =>
    cb(ev.payload),
  );
}

export async function onAppState(
  cb: (s: AppSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<AppSnapshot>("app-state", (e) => cb(e.payload));
}

export async function onSettingsChanged(
  cb: (c: AppConfig) => void,
): Promise<UnlistenFn> {
  return listen<AppConfig>("settings-changed", (e) => cb(e.payload));
}

export async function onAppError(
  cb: (e: AppErrorEvent) => void,
): Promise<UnlistenFn> {
  return listen<AppErrorEvent>("app-error", (ev) => cb(ev.payload));
}

export type BrightnessSnap = { id: string; brightness: number };

export async function onUiShowFlyout(
  cb: (
    page?: string,
    quiet?: boolean,
    brightness?: BrightnessSnap[],
  ) => void,
): Promise<UnlistenFn> {
  return listen<{
    page?: string;
    quiet?: boolean;
    brightness?: BrightnessSnap[];
  } | string | null>("ui-show-flyout", (e) => {
    const p = e.payload;
    if (p && typeof p === "object") {
      cb(
        "page" in p ? p.page : "flyout",
        "quiet" in p ? !!p.quiet : false,
        Array.isArray(p.brightness) ? p.brightness : undefined,
      );
    } else if (typeof p === "string") {
      cb(p, false, undefined);
    } else {
      cb("flyout", false, undefined);
    }
  });
}

export async function onUiToggleFlyout(cb: () => void): Promise<UnlistenFn> {
  return listen("ui-toggle-flyout", () => cb());
}

export async function onWindowHidden(cb: () => void): Promise<UnlistenFn> {
  return listen("window-hidden", () => cb());
}

export async function onWindowShown(cb: () => void): Promise<UnlistenFn> {
  return listen("window-shown", () => cb());
}
