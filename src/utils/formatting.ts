import { t } from "../i18n";

export function formatPercent(n: number): string {
  return `${Math.round(Math.min(100, Math.max(0, n)))}%`;
}

export function shortId(id: string): string {
  if (id.length <= 14) return id;
  return `${id.slice(0, 8)}…${id.slice(-4)}`;
}

export function formatHotkey(accel: string | null | undefined): string {
  if (!accel) return t("hotkey.unset");
  return accel
    .replace(/ArrowUp/g, "↑")
    .replace(/ArrowDown/g, "↓")
    .replace(/ArrowLeft/g, "←")
    .replace(/ArrowRight/g, "→")
    .replace(/Super/g, "Win");
}
