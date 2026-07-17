import { t } from "../i18n";
import type { MonitorInfo } from "../state/types";
import { monitorLabel, statusLabel } from "../state/types";
import { formatPercent } from "../utils/formatting";
import { BrightnessSlider } from "./brightness-slider";
import { iconMonitor } from "./icons";

export interface MonitorListOptions {
  monitors: MonitorInfo[];
  step: number;
  onBrightness: (id: string, value: number, finalWrite: boolean) => void;
  onSelect?: (id: string) => void;
  onDragState?: (dragging: boolean) => void;
  collectValueEl?: (id: string, el: HTMLElement) => void;
}

export function renderMonitorList(
  container: HTMLElement,
  opts: MonitorListOptions,
): BrightnessSlider[] {
  container.innerHTML = "";
  container.className = "flyout-list";
  const sliders: BrightnessSlider[] = [];

  for (const m of opts.monitors) {
    const row = document.createElement("div");
    row.className = "flyout-row";
    row.dataset.id = m.id;
    row.innerHTML = `
      <div class="flyout-row__icon">${iconMonitor}</div>
      <div class="flyout-row__name" title="${escapeAttr(m.description)}">${escapeHtml(monitorLabel(m))}</div>
      <div class="flyout-row__value">${formatPercent(m.cachedBrightness)}</div>
      <div class="flyout-row__slider"></div>
    `;
    if (opts.onSelect) {
      row.querySelector(".flyout-row__name")!.addEventListener("click", () => {
        opts.onSelect?.(m.id);
      });
    }
    const slot = row.querySelector(".flyout-row__slider") as HTMLElement;
    const valueEl = row.querySelector(".flyout-row__value") as HTMLElement;
    opts.collectValueEl?.(m.id, valueEl);
    const slider = new BrightnessSlider({
      value: m.cachedBrightness,
      step: opts.step,
      disabled: !m.isControllable,
      ariaLabel: t("flyout.brightnessOf", { name: monitorLabel(m) }),
      onDragState: (d) => opts.onDragState?.(d),
      onPreview: (v) => {
        valueEl.textContent = formatPercent(v);
      },
      onInput: (v, finalWrite) => {
        valueEl.textContent = formatPercent(v);
        opts.onBrightness(m.id, v, finalWrite);
      },
    });
    slot.appendChild(slider.el);
    sliders.push(slider);

    if (!m.isControllable) {
      const st = document.createElement("div");
      st.className = "flyout__status is-warn";
      st.style.gridColumn = "2 / span 2";
      st.textContent = statusLabel(m.status);
      row.appendChild(st);
    }

    container.appendChild(row);
  }

  return sliders;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/'/g, "&#39;");
}
