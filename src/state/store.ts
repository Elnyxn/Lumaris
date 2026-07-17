import type { AppConfig, MonitorInfo, PageId } from "./types";

export type Listener = () => void;

export interface AppStore {
  ready: boolean;
  visible: boolean;
  page: PageId;
  config: AppConfig | null;
  monitors: MonitorInfo[];
  selectedId: string | null;
  autostartEnabled: boolean;
  version: string;
  toast: string | null;
  statusMessage: string | null;
  recordingAction: string | null;
}

const state: AppStore = {
  ready: false,
  visible: false,
  page: "flyout",
  config: null,
  monitors: [],
  selectedId: null,
  autostartEnabled: false,
  version: "1.0.0",
  toast: null,
  statusMessage: null,
  recordingAction: null,
};

const listeners = new Set<Listener>();

export function getState(): Readonly<AppStore> {
  return state;
}

export function subscribe(fn: Listener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function emit(): void {
  for (const fn of listeners) fn();
}

export function patch(partial: Partial<AppStore>): void {
  Object.assign(state, partial);
  if (partial.monitors) {
    const sel = partial.monitors.find((m) => m.isSelected);
    state.selectedId = sel?.id ?? partial.monitors[0]?.id ?? null;
  }
  emit();
}

export function setMonitors(monitors: MonitorInfo[]): void {
  state.monitors = monitors;
  const sel = monitors.find((m) => m.isSelected);
  state.selectedId = sel?.id ?? monitors[0]?.id ?? state.selectedId;
  emit();
}

export function updateMonitorBrightness(
  id: string,
  brightness: number,
  status?: string,
  silent = false,
): void {
  state.monitors = state.monitors.map((m) =>
    m.id === id
      ? {
          ...m,
          currentBrightness: brightness,
          cachedBrightness: brightness,
          status: (status as MonitorInfo["status"]) ?? m.status,
        }
      : m,
  );
  // silent：拖动中改缓存但不通知订阅者，避免整页重绘
  if (!silent) emit();
}

export function updateMonitorContrast(
  id: string,
  contrast: number,
  silent = false,
): void {
  state.monitors = state.monitors.map((m) =>
    m.id === id
      ? {
          ...m,
          currentContrast: contrast,
          cachedContrast: contrast,
        }
      : m,
  );
  if (!silent) emit();
}

export function selectedMonitor(): MonitorInfo | null {
  if (!state.selectedId) return state.monitors[0] ?? null;
  return state.monitors.find((m) => m.id === state.selectedId) ?? null;
}

export function showToast(message: string, ms = 2200): void {
  state.toast = message;
  emit();
  window.setTimeout(() => {
    if (state.toast === message) {
      state.toast = null;
      emit();
    }
  }, ms);
}
