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
  onSdrBrightness?: (id: string, value: number, finalWrite: boolean) => void;
  onSelect?: (id: string) => void;
  onDragState?: (dragging: boolean) => void;
  collectValueEl?: (id: string, el: HTMLElement) => void;
  collectSdrSlider?: (id: string, slider: BrightnessSlider, valueEl: HTMLElement) => void;
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

    if (m.sdrControllable && opts.onSdrBrightness) {
      const sdrLabel = document.createElement("div");
      sdrLabel.className = "flyout-row__sdr-label";
      sdrLabel.textContent = "SDR";
      const sdrValue = document.createElement("div");
      sdrValue.className = "flyout-row__value flyout-row__sdr-value";
      sdrValue.textContent = formatPercent(m.sdrBrightness);
      const sdrSlot = document.createElement("div");
      sdrSlot.className = "flyout-row__slider flyout-row__sdr-slider";
      row.append(sdrLabel, sdrValue, sdrSlot);
      const sdrSlider = new BrightnessSlider({
        value: m.sdrBrightness,
        step: opts.step,
        ariaLabel: t("flyout.sdrBrightnessOf", { name: monitorLabel(m) }),
        onDragState: (d) => opts.onDragState?.(d),
        onPreview: (v) => {
          sdrValue.textContent = formatPercent(v);
        },
        onInput: (v, finalWrite) => {
          sdrValue.textContent = formatPercent(v);
          opts.onSdrBrightness?.(m.id, v, finalWrite);
        },
      });
      sdrSlot.appendChild(sdrSlider.el);
      opts.collectSdrSlider?.(m.id, sdrSlider, sdrValue);
    }

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
