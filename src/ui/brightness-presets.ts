import { t } from "../i18n";
import { getState } from "../state/store";
import { monitorLabel, type AppConfig, type BrightnessPresetMode, type MonitorInfo } from "../state/types";

interface PresetHandlers {
  onSave: (mode: BrightnessPresetMode, brightness: Record<string, number>) => Promise<AppConfig>;
  onApply: (mode: BrightnessPresetMode) => Promise<void>;
  onToast: (message: string) => void;
}

/** 编辑仅修改草稿，保存与硬件应用分别由显式动作触发。 */
export class BrightnessPresetEditor {
  readonly root = document.createElement("div");
  private mode: BrightnessPresetMode = "day";
  private drafts: AppConfig["brightnessPresets"] = { day: {}, night: {} };
  private baseline: AppConfig["brightnessPresets"] = { day: {}, night: {} };
  private dirty = { day: new Set<string>(), night: new Set<string>() };
  private monitors: MonitorInfo[] = [];
  private rows = new Map<string, { range: HTMLInputElement; number: HTMLInputElement }>();
  private structureKey = "";
  private busy = false;
  private status: HTMLElement | null = null;

  constructor(private handlers: PresetHandlers) {
    this.root.className = "preset-editor";
  }

  update(config: AppConfig, monitors: MonitorInfo[]): void {
    this.monitors = monitors.filter((m) => m.isControllable &&
      ["available", "cached", "readFailed", "writeFailed"].includes(m.status));
    for (const mode of ["day", "night"] as const) {
      const saved = config.brightnessPresets?.[mode] ?? {};
      const previous = this.baseline[mode];
      const draft = this.drafts[mode];
      for (const id of new Set([...Object.keys(draft), ...Object.keys(saved)])) {
        // 新保存的数据优先于旧草稿；未变化的字段保留用户未保存的编辑。
        if (saved[id] !== previous[id] || !this.dirty[mode].has(id)) {
          const monitor = this.monitors.find((m) => m.id === id);
          if (saved[id] !== undefined) draft[id] = saved[id];
          else if (monitor) draft[id] = monitor.cachedBrightness;
          else delete draft[id];
          this.dirty[mode].delete(id);
        }
      }
      for (const monitor of this.monitors) {
        draft[monitor.id] ??= saved[monitor.id] ?? monitor.cachedBrightness;
      }
      this.baseline[mode] = { ...saved };
    }
    const key = JSON.stringify([
      this.mode, config.locale,
      this.monitors.map((m) => [m.id, monitorLabel(m)]),
    ]);
    if (key !== this.structureKey) {
      this.structureKey = key;
      this.render();
    } else {
      this.paint();
    }
  }

  private render(): void {
    this.rows.clear();
    this.root.innerHTML = "";
    const tabs = document.createElement("div");
    tabs.className = "preset-editor__tabs";
    tabs.setAttribute("role", "tablist");
    tabs.setAttribute("aria-label", t("settings.brightnessPresets"));
    for (const mode of ["day", "night"] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "preset-editor__tab";
      button.textContent = t(`preset.${mode}`);
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", String(mode === this.mode));
      button.tabIndex = mode === this.mode ? 0 : -1;
      const select = (): void => {
        this.mode = mode;
        const config = getState().config;
        if (config) this.update(config, getState().monitors);
        this.root.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
      };
      button.addEventListener("click", select);
      button.addEventListener("keydown", (event) => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        this.mode = event.key === "Home" ? "day" : event.key === "End" ? "night" : this.mode === "day" ? "night" : "day";
        const config = getState().config;
        if (config) this.update(config, getState().monitors);
        this.root.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
      });
      tabs.appendChild(button);
    }
    this.root.appendChild(tabs);

    const panel = document.createElement("div");
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-label", t(`preset.${this.mode}`));
    for (const monitor of this.monitors) {
      const row = document.createElement("div");
      row.className = "preset-editor__row";
      const label = document.createElement("div");
      label.className = "preset-editor__name";
      label.textContent = monitorLabel(monitor);
      label.title = label.textContent;
      const controls = document.createElement("div");
      controls.className = "preset-editor__controls";
      const range = document.createElement("input");
      range.type = "range";
      range.className = "preset-editor__range";
      const number = document.createElement("input");
      number.type = "number";
      number.className = "input preset-editor__number";
      for (const input of [range, number]) {
        input.min = "0";
        input.max = "100";
        input.step = "1";
        input.setAttribute("aria-label", t("flyout.brightnessOf", { name: monitorLabel(monitor) }));
      }
      range.addEventListener("input", () => this.edit(monitor.id, range.valueAsNumber));
      number.addEventListener("input", () => {
        if (Number.isFinite(number.valueAsNumber)) this.edit(monitor.id, number.valueAsNumber, false);
      });
      number.addEventListener("change", () => this.edit(monitor.id,
        Number.isFinite(number.valueAsNumber) ? number.valueAsNumber : this.drafts[this.mode][monitor.id]));
      const percent = document.createElement("span");
      percent.textContent = "%";
      percent.setAttribute("aria-hidden", "true");
      controls.append(range, number, percent);
      row.append(label, controls);
      panel.appendChild(row);
      this.rows.set(monitor.id, { range, number });
    }
    this.status = document.createElement("div");
    this.status.className = "preset-editor__status";
    this.status.setAttribute("role", "status");
    panel.appendChild(this.status);
    const actions = document.createElement("div");
    actions.className = "preset-editor__actions";
    for (const action of ["readCurrent", "save", "apply"] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "text-btn";
      button.dataset.action = action;
      button.textContent = t(`preset.${action}`);
      button.addEventListener("click", () => {
        if (action === "readCurrent") this.readCurrent();
        else void this.commit(action === "apply");
      });
      actions.appendChild(button);
    }
    panel.appendChild(actions);
    this.root.appendChild(panel);
    this.paint();
  }

  private edit(id: string, value: number, normalizeNumber = true): void {
    const next = Math.round(Math.min(100, Math.max(0, value)));
    this.drafts[this.mode][id] = next;
    if (next === this.baseline[this.mode][id]) this.dirty[this.mode].delete(id);
    else this.dirty[this.mode].add(id);
    const row = this.rows.get(id);
    if (row) {
      row.range.value = String(next);
      if (normalizeNumber) row.number.value = String(next);
    }
    this.paintStatus();
  }

  private readCurrent(): void {
    const current = new Map(getState().monitors.map((m) => [m.id, m.cachedBrightness]));
    for (const monitor of this.monitors) {
      this.edit(monitor.id, current.get(monitor.id) ?? monitor.cachedBrightness);
    }
  }

  private async commit(apply: boolean): Promise<void> {
    if (this.busy || !this.monitors.length) return;
    const config = getState().config;
    if (!config?.brightnessPresetsEnabled) return;
    this.update(config, getState().monitors);
    if (!this.monitors.length) return;
    const mode = this.mode;
    const brightness = Object.fromEntries(this.monitors.map((m) => [m.id, this.drafts[mode][m.id]]));
    this.busy = true;
    this.paint();
    try {
      const saved = await this.handlers.onSave(mode, brightness);
      this.update(saved, getState().monitors);
      if (apply) await this.handlers.onApply(mode);
    } catch (error) {
      this.handlers.onToast(error instanceof Error ? error.message : String(error));
    } finally {
      this.busy = false;
      const current = getState().config;
      if (current) this.update(current, getState().monitors);
      else this.paint();
    }
  }

  private paintStatus(): void {
    if (!this.status) return;
    const count = Object.keys(this.baseline[this.mode]).length;
    this.status.textContent = !this.monitors.length ? t("preset.noMonitors")
      : this.monitors.some((m) => this.dirty[this.mode].has(m.id)) ? t("preset.unsavedChanges")
      : count ? t("preset.savedCount", { n: count }) : t("preset.notSaved");
  }

  private paint(): void {
    for (const [id, inputs] of this.rows) {
      const value = String(this.drafts[this.mode][id]);
      inputs.range.value = value;
      if (document.activeElement !== inputs.number || this.busy) inputs.number.value = value;
      inputs.range.disabled = this.busy;
      inputs.number.disabled = this.busy;
    }
    this.root.querySelectorAll<HTMLButtonElement>("button").forEach((button) => {
      button.disabled = this.busy || (button.dataset.action !== undefined && !this.monitors.length);
    });
    this.paintStatus();
  }
}
