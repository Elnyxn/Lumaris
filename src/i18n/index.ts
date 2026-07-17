import { en } from "./locales/en";
import { zhCN, type MessageKey, type Messages } from "./locales/zh-CN";

export type LocaleId = "zh-CN" | "en";

export type { MessageKey, Messages };

const PACKS: Record<LocaleId, Messages> = {
  "zh-CN": zhCN,
  en,
};

export const LOCALES: { id: LocaleId; labelKey: MessageKey }[] = [
  { id: "zh-CN", labelKey: "ui.lang.zhCN" },
  { id: "en", labelKey: "ui.lang.en" },
];

let current: LocaleId = "zh-CN";
const listeners = new Set<() => void>();

export function normalizeLocale(raw: string | null | undefined): LocaleId {
  if (!raw) return "zh-CN";
  const s = raw.trim().toLowerCase().replace("_", "-");
  if (s === "en" || s.startsWith("en-")) return "en";
  if (s === "zh" || s.startsWith("zh-")) return "zh-CN";
  if (raw === "zh-CN" || raw === "en") return raw;
  return "zh-CN";
}

export function getLocale(): LocaleId {
  return current;
}

export function setLocale(locale: string): LocaleId {
  const next = normalizeLocale(locale);
  if (next === current) return current;
  current = next;
  document.documentElement.lang = next === "zh-CN" ? "zh-CN" : "en";
  for (const fn of listeners) fn();
  return current;
}

export function onLocaleChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** 简单插值：`t("key", { n: 2 })` → 替换 `{n}` */
export function t(
  key: MessageKey,
  vars?: Record<string, string | number>,
): string {
  const pack = PACKS[current] ?? zhCN;
  let s: string = pack[key] ?? zhCN[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
    }
  }
  return s;
}

export function hotkeyLabel(
  id:
    | "increase"
    | "decrease"
    | "toggleFlyout"
    | "prevMonitor"
    | "nextMonitor"
    | "syncIncrease"
    | "syncDecrease",
): string {
  const map = {
    increase: "hotkey.increase",
    decrease: "hotkey.decrease",
    toggleFlyout: "hotkey.toggleFlyout",
    prevMonitor: "hotkey.prevMonitor",
    nextMonitor: "hotkey.nextMonitor",
    syncIncrease: "hotkey.syncIncrease",
    syncDecrease: "hotkey.syncDecrease",
  } as const;
  return t(map[id]);
}
