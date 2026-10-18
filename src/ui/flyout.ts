import { t } from "../i18n";
import { getState, selectedMonitor } from "../state/store";
import { monitorLabel, statusLabel, type BrightnessPresetMode } from "../state/types";
import { formatPercent } from "../utils/formatting";
import { BrightnessSlider } from "./brightness-slider";
import { renderMonitorList } from "./monitor-list";
import { iconContrast, iconMonitor, iconSettings, iconSun } from "./icons";

export interface FlyoutHandlers {
  onBrightness: (id: string, value: number, finalWrite: boolean) => void;
  onContrast: (id: string, value: number, finalWrite: boolean) => void;
  onSdrBrightness: (id: string, value: number, finalWrite: boolean) => void;
  onPrev: () => void;
  onNext: () => void;
  onSettings: () => void;
  onApplyBrightnessPreset: (mode: BrightnessPresetMode) => Promise<void>;
  onSelect: (id: string) => void;
  onInteract: () => void;
  onLayout: () => void;
  onDragState?: (dragging: boolean) => void;
}

export class FlyoutView {
  readonly root: HTMLElement;
  private slider: BrightnessSlider | null = null;
  private contrastSlider: BrightnessSlider | null = null;
  private sdrSlider: BrightnessSlider | null = null;
  private valueEl: HTMLElement | null = null;
  private contrastValueEl: HTMLElement | null = null;
  private sdrValueEl: HTMLElement | null = null;
  private listSliders: BrightnessSlider[] = [];
  private listValueEls = new Map<string, HTMLElement>();
  private listSdrSliders = new Map<string, BrightnessSlider>();
  private listSdrValueEls = new Map<string, HTMLElement>();
  private handlers: FlyoutHandlers;
  private boundId: string | null = null;
  private layoutObserver: ResizeObserver;

  constructor(handlers: FlyoutHandlers) {
    this.handlers = handlers;
    this.root = document.createElement("div");
    this.root.className = "flyout";
    this.root.addEventListener("pointerdown", () => handlers.onInteract());
    this.layoutObserver = new ResizeObserver(() => this.handlers.onLayout());
  }

  /** 与窗口当前高度无关：列表按实际行高求和，不能用会包含空白的 scrollHeight。 */
  naturalHeight(): number {
    if (!this.root.isConnected) return 0;
    const outerHeight = (el: Element): number => {
      const style = getComputedStyle(el);
      return el.getBoundingClientRect().height +
        parseFloat(style.marginTop || "0") + parseFloat(style.marginBottom || "0");
    };
    const children = [...this.root.children];
    const style = getComputedStyle(this.root);
    let height = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom) +
      parseFloat(style.borderTopWidth) + parseFloat(style.borderBottomWidth) +
      Math.max(0, children.length - 1) * parseFloat(style.rowGap || "0");
    for (const child of children) {
      if (child.matches("[data-list]")) {
        const rows = [...child.children];
        const listStyle = getComputedStyle(child);
        height += rows.reduce((sum, row) => sum + outerHeight(row), 0) +
          Math.max(0, rows.length - 1) * parseFloat(listStyle.rowGap || "0");
      } else {
        height += outerHeight(child);
      }
    }
    return Math.ceil(height);
  }

  destroy(): void {
    this.layoutObserver.disconnect();
  }

  isDragging(): boolean {
    if (
      this.slider?.isDragging() ||
      this.contrastSlider?.isDragging() ||
      this.sdrSlider?.isDragging()
    ) {
      return true;
    }
    return (
      this.listSliders.some((s) => s.isDragging()) ||
      [...this.listSdrSliders.values()].some((s) => s.isDragging())
    );
  }

  render(): void {
    if (this.isDragging()) return;

    const st = getState();
    const cfg = st.config;
    const step = cfg?.stepPercent || cfg?.customStepPercent || 5;
    // 多显示器：强制上下堆叠，不再左右切换
    const multi = st.monitors.length > 1;
    this.root.classList.toggle("flyout--single", !multi);
    if (multi) {
      this.renderList(step);
    } else {
      this.renderSingle(step);
    }
    const presets = document.createElement("div");
    presets.className = "flyout__presets";
    for (const mode of ["day", "night"] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "text-btn flyout__preset";
      button.textContent = t(`preset.${mode}`);
      button.setAttribute("aria-label", t("preset.applyAria", { mode: t(`preset.${mode}`) }));
      button.disabled = !Object.keys(cfg?.brightnessPresets?.[mode] ?? {}).length;
      button.addEventListener("click", async () => {
        button.disabled = true;
        try {
          await this.handlers.onApplyBrightnessPreset(mode);
        } finally {
          button.disabled = !Object.keys(getState().config?.brightnessPresets?.[mode] ?? {}).length;
        }
      });
      presets.appendChild(button);
    }
    const footer = this.root.querySelector(".flyout__footer")!;
    footer.insertBefore(presets, footer.querySelector('[data-act="settings"]'));
    this.layoutObserver.disconnect();
    this.layoutObserver.observe(this.root);
    // 字体加载、状态换行和 SDR 行都可能改变自然高度，即使窗口已达到上限。
    this.root.querySelectorAll(".flyout__header, .flyout__body, .flyout__footer, .flyout-row")
      .forEach((el) => this.layoutObserver.observe(el));
    this.handlers.onLayout();
  }

  private renderSingle(step: number): void {
    const m = selectedMonitor();
    // 对比度仅单屏且开启时显示；多屏堆叠从不显示
    const showContrast =
      getState().monitors.length <= 1 &&
      getState().config?.showContrast === true;
    this.boundId = m?.id ?? null;

    const contrastRow =
      showContrast
        ? `
      <div class="flyout__body flyout__body--contrast">
        <div aria-hidden="true">${iconContrast}</div>
        <div data-cslider class="flyout__slider-slot"></div>
        <div class="flyout__value" data-cvalue>${m ? formatPercent(m.cachedContrast ?? 50) : "—"}</div>
      </div>`
        : "";
    const sdrRow = m?.sdrControllable
      ? `
      <div class="flyout__body flyout__body--sdr">
        <div class="flyout__sdr-label" aria-hidden="true">SDR</div>
        <div data-sdr-slider class="flyout__slider-slot"></div>
        <div class="flyout__value" data-sdr-value>${formatPercent(m.sdrBrightness)}</div>
      </div>`
      : "";

    this.root.innerHTML = `
      <div class="flyout__header">
        <div aria-hidden="true">${iconMonitor}</div>
        <div class="flyout__title">
          <div class="flyout__name" title="${m ? escapeAttr(m.description) : ""}">${m ? escapeHtml(monitorLabel(m)) : escapeHtml(t("flyout.noMonitor"))}</div>
          <div class="flyout__status" data-status></div>
        </div>
      </div>
      <div class="flyout__body">
        <div aria-hidden="true">${iconSun}</div>
        <div data-slider class="flyout__slider-slot"></div>
        <div class="flyout__value" data-value>${m ? formatPercent(m.cachedBrightness) : "—"}</div>
      </div>
      ${sdrRow}
      ${contrastRow}
      <div class="flyout__footer">
        <span class="flyout__footer-label" aria-hidden="true">${escapeHtml(showContrast ? t("flyout.brightnessContrast") : t("flyout.adjustBrightness"))}</span>
        <button type="button" class="icon-btn" data-act="settings" aria-label="${escapeAttr(t("flyout.settingsAria"))}">${iconSettings}</button>
      </div>
    `;

    const statusEl = this.root.querySelector("[data-status]") as HTMLElement;
    if (m) {
      statusEl.textContent = statusLabel(m.status);
      statusEl.classList.toggle(
        "is-error",
        /fail|disconnect|unsupported/i.test(String(m.status)),
      );
      statusEl.classList.toggle(
        "is-warn",
        /cached|offline|sleep/i.test(String(m.status)),
      );
    } else {
      statusEl.textContent = t("flyout.noMonitorDetected");
      statusEl.classList.add("is-warn");
    }

    const slot = this.root.querySelector("[data-slider]") as HTMLElement;
    this.valueEl = this.root.querySelector("[data-value]") as HTMLElement;

    this.slider = new BrightnessSlider({
      value: m?.cachedBrightness ?? 0,
      step,
      disabled: !m,
      ariaLabel: t("flyout.brightnessAria"),
      onDragState: (d) => this.handlers.onDragState?.(d),
      onPreview: (v) => {
        if (this.valueEl) this.valueEl.textContent = formatPercent(v);
      },
      onInput: (v, finalWrite) => {
        if (this.valueEl) this.valueEl.textContent = formatPercent(v);
        if (m) this.handlers.onBrightness(m.id, v, finalWrite);
        this.handlers.onInteract();
      },
    });
    slot.appendChild(this.slider.el);

    this.sdrSlider = null;
    this.sdrValueEl = null;
    if (m?.sdrControllable) {
      const sdrSlot = this.root.querySelector("[data-sdr-slider]") as HTMLElement;
      this.sdrValueEl = this.root.querySelector("[data-sdr-value]") as HTMLElement;
      this.sdrSlider = new BrightnessSlider({
        value: m.sdrBrightness,
        step,
        ariaLabel: t("flyout.sdrBrightnessAria"),
        onDragState: (d) => this.handlers.onDragState?.(d),
        onPreview: (v) => {
          if (this.sdrValueEl) this.sdrValueEl.textContent = formatPercent(v);
        },
        onInput: (v, finalWrite) => {
          if (this.sdrValueEl) this.sdrValueEl.textContent = formatPercent(v);
          this.handlers.onSdrBrightness(m.id, v, finalWrite);
          this.handlers.onInteract();
        },
      });
      sdrSlot.appendChild(this.sdrSlider.el);
    }

    this.contrastSlider = null;
    this.contrastValueEl = null;
    if (showContrast) {
      const cslot = this.root.querySelector("[data-cslider]") as HTMLElement;
      this.contrastValueEl = this.root.querySelector(
        "[data-cvalue]",
      ) as HTMLElement;
      this.contrastSlider = new BrightnessSlider({
        value: m?.cachedContrast ?? 50,
        step,
        disabled: !m || m.contrastControllable === false,
        ariaLabel: t("flyout.contrastAria"),
        onDragState: (d) => this.handlers.onDragState?.(d),
        onPreview: (v) => {
          if (this.contrastValueEl)
            this.contrastValueEl.textContent = formatPercent(v);
        },
        onInput: (v, finalWrite) => {
          if (this.contrastValueEl)
            this.contrastValueEl.textContent = formatPercent(v);
          if (m) this.handlers.onContrast(m.id, v, finalWrite);
          this.handlers.onInteract();
        },
      });
      cslot.appendChild(this.contrastSlider.el);
    }

    this.root
      .querySelector('[data-act="settings"]')!
      .addEventListener("click", () => this.handlers.onSettings());
  }

  private renderList(step: number): void {
    const n = getState().monitors.length;
    this.root.innerHTML = `
      <div class="flyout__header">
        <div aria-hidden="true">${iconMonitor}</div>
        <div class="flyout__title">
          <div class="flyout__name">${escapeHtml(t("flyout.monitors"))}</div>
          <div class="flyout__status">${escapeHtml(t("flyout.monitorsCount", { n }))}</div>
        </div>
      </div>
      <div data-list class="flyout-list-host"></div>
      <div class="flyout__footer">
        <span class="flyout__footer-label">${escapeHtml(t("flyout.adjustBrightness"))}</span>
        <button type="button" class="icon-btn" data-act="settings" aria-label="${escapeAttr(t("flyout.settingsAria"))}">${iconSettings}</button>
      </div>
    `;
    const list = this.root.querySelector("[data-list]") as HTMLElement;
    this.slider = null;
    this.contrastSlider = null;
    this.sdrSlider = null;
    this.valueEl = null;
    this.contrastValueEl = null;
    this.sdrValueEl = null;
    this.boundId = null;
    this.listValueEls.clear();
    this.listSdrSliders.clear();
    this.listSdrValueEls.clear();
    this.listSliders = renderMonitorList(list, {
      monitors: getState().monitors,
      step,
      onBrightness: (id, v, finalWrite) => {
        this.handlers.onBrightness(id, v, finalWrite);
        this.handlers.onInteract();
      },
      onSdrBrightness: (id, v, finalWrite) => {
        this.handlers.onSdrBrightness(id, v, finalWrite);
        this.handlers.onInteract();
      },
      onSelect: (id) => this.handlers.onSelect(id),
      onDragState: (d) => this.handlers.onDragState?.(d),
      collectValueEl: (id, el) => {
        this.listValueEls.set(id, el);
      },
      collectSdrSlider: (id, slider, valueEl) => {
        this.listSdrSliders.set(id, slider);
        this.listSdrValueEls.set(id, valueEl);
      },
    });
    this.root
      .querySelector('[data-act="settings"]')!
      .addEventListener("click", () => this.handlers.onSettings());
  }

  updateBrightnessVisual(id: string, value: number): void {
    if (this.isDragging()) return;
    if (this.slider && this.boundId === id) {
      this.slider.setValue(value, false);
      if (this.valueEl) this.valueEl.textContent = formatPercent(value);
      return;
    }
    // 堆叠列表：按 id 更新对应滑条
    const st = getState();
    const idx = st.monitors.findIndex((m) => m.id === id);
    if (idx >= 0 && this.listSliders[idx]) {
      this.listSliders[idx].setValue(value, false);
      const vel = this.listValueEls.get(id);
      if (vel) vel.textContent = formatPercent(value);
    }
  }

  updateContrastVisual(id: string, value: number): void {
    if (this.isDragging()) return;
    if (this.contrastSlider && this.boundId === id) {
      this.contrastSlider.setValue(value, false);
      if (this.contrastValueEl)
        this.contrastValueEl.textContent = formatPercent(value);
    }
  }


  updateSdrBrightnessVisual(id: string, value: number): void {
    if (this.isDragging()) return;
    if (this.sdrSlider && this.boundId === id) {
      this.sdrSlider.setValue(value, false);
      if (this.sdrValueEl) this.sdrValueEl.textContent = formatPercent(value);
      return;
    }
    const slider = this.listSdrSliders.get(id);
    slider?.setValue(value, false);
    const valueEl = this.listSdrValueEls.get(id);
    if (valueEl) valueEl.textContent = formatPercent(value);
  }
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
