import { hotkeyLabel, LOCALES, normalizeLocale, t } from "../i18n";
import { getState } from "../state/store";
import {
  HOTKEY_ACTIONS,
  controlMethodLabel,
  hotkeyActionToRust,
  monitorLabel,
  statusLabel,
  type HotkeyConfig,
  type TargetMode,
} from "../state/types";
import { shortId } from "../utils/formatting";
import * as cmd from "../ipc/commands";
import { HotkeyRecorder } from "./hotkey-recorder";
import { iconBack, iconGithub } from "./icons";
import { FluentSelect } from "./select";

const GITHUB_URL = "https://github.com/Elnyxn/Lumaris";
const GITHUB_DISPLAY = "github.com/Elnyxn/Lumaris";

export interface SettingsHandlers {
  onBack: () => void;
  onPatchConfig: (patch: Record<string, unknown>) => Promise<void>;
  onSetHotkey: (action: string, accel: string | null) => Promise<string | null>;
  onResetHotkeys: () => Promise<void>;
  onSetAutostart: (enabled: boolean) => Promise<void>;
  onAlias: (id: string, alias: string | null) => Promise<void>;
  onSyncInclude: (id: string, include: boolean) => Promise<void>;
  onOpenLogs: () => Promise<void>;
  onResetAll: () => Promise<void>;
  onRefresh: () => Promise<void>;
  onRecordingChange: (recording: boolean) => void;
  onToast?: (message: string) => void;
}

export class SettingsView {
  readonly root: HTMLElement;
  private handlers: SettingsHandlers;
  private recorders: HotkeyRecorder[] = [];
  private selects: FluentSelect[] = [];
  private configPatchQueue: Promise<void> = Promise.resolve();

  constructor(handlers: SettingsHandlers) {
    this.handlers = handlers;
    this.root = document.createElement("div");
    this.root.className = "settings";
  }

  /** 串行化配置写入，确保后一项基于前一项已落盘的最新配置合并。 */
  private queuePatchConfig(patch: Record<string, unknown>): Promise<void> {
    const run = this.configPatchQueue.then(() =>
      this.handlers.onPatchConfig(patch),
    );
    this.configPatchQueue = run.catch(() => undefined);
    return run;
  }

  render(): void {
    this.destroyRecorders();
    this.destroySelects();
    const st = getState();
    const cfg = st.config;
    if (!cfg) {
      this.root.innerHTML = `<div class="settings__header"><div class="settings__title">${escapeHtml(t("settings.title"))}</div></div>`;
      return;
    }

    this.root.innerHTML = `
      <div class="settings__header">
        <button type="button" class="icon-btn" data-act="back" aria-label="${escapeAttr(t("settings.back"))}">${iconBack}</button>
        <div class="settings__title">${escapeHtml(t("settings.title"))}</div>
      </div>
      <div class="settings__body scrollable">
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.general"))}</div>
          <div class="settings__card" data-sec="general"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.brightness"))}</div>
          <div class="settings__card" data-sec="brightness"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.hotkeys"))}</div>
          <div class="settings__card" data-sec="hotkeys"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.monitors"))}</div>
          <div class="settings__card" data-sec="monitors"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.ui"))}</div>
          <div class="settings__card" data-sec="ui"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("settings.advanced"))}</div>
          <div class="settings__card" data-sec="advanced"></div>
        </section>
        <section class="settings__section">
          <div class="settings__section-title">${escapeHtml(t("about.title"))}</div>
          <div class="settings__card" data-sec="about"></div>
        </section>
      </div>
      <div class="settings__footer">
        <span class="settings__version">${escapeHtml(t("settings.version", { v: st.version }))}</span>
        <button type="button" class="text-btn danger" data-act="reset">${escapeHtml(t("settings.resetAll"))}</button>
      </div>
    `;

    this.root
      .querySelector('[data-act="back"]')!
      .addEventListener("click", () => this.handlers.onBack());
    this.root
      .querySelector('[data-act="reset"]')!
      .addEventListener("click", () => void this.handlers.onResetAll());

    this.fillGeneral(cfg);
    this.fillBrightness(cfg);
    this.fillHotkeys(cfg.hotkeys);
    this.fillMonitors();
    this.fillUi(cfg);
    this.fillAdvanced(cfg);
    this.fillAbout();
  }

  private fillGeneral(cfg: NonNullable<ReturnType<typeof getState>["config"]>): void {
    const el = this.root.querySelector('[data-sec="general"]') as HTMLElement;
    el.appendChild(
      rowSwitch(t("settings.autostart"), getState().autostartEnabled, (v) =>
        this.handlers.onSetAutostart(v),
      ),
    );
    el.appendChild(
      rowSwitch(t("settings.silentStartup"), cfg.silentStartup, (v) =>
        this.queuePatchConfig({ silentStartup: v }),
      ),
    );
    el.appendChild(
      rowSwitch(t("settings.delayedInit"), cfg.delayedMonitorInit, (v) =>
        this.queuePatchConfig({ delayedMonitorInit: v }),
      ),
    );
  }

  private fillBrightness(
    cfg: NonNullable<ReturnType<typeof getState>["config"]>,
  ): void {
    const el = this.root.querySelector('[data-sec="brightness"]') as HTMLElement;

    const stepField = document.createElement("div");
    stepField.className = "field";
    stepField.innerHTML = `
      <div>
        <div class="field__label">${escapeHtml(t("settings.step"))}</div>
        <div class="field__hint">${escapeHtml(t("settings.stepHint"))}</div>
      </div>
    `;
    const stepSelect = new FluentSelect({
      value: String([1, 2, 5, 10].includes(cfg.stepPercent) ? cfg.stepPercent : 0),
      options: [
        { value: "1", label: "1%" },
        { value: "2", label: "2%" },
        { value: "5", label: "5%" },
        { value: "10", label: "10%" },
        { value: "0", label: t("settings.stepCustom") },
      ],
      ariaLabel: t("settings.step"),
      minWidth: 100,
      onChange: (v) => {
        void this.queuePatchConfig({ stepPercent: Number(v) });
      },
    });
    this.selects.push(stepSelect);
    stepField.appendChild(stepSelect.el);
    el.appendChild(stepField);

    el.appendChild(
      rowSwitch(t("settings.osd"), cfg.osd.enabled, (v) =>
        this.queuePatchConfig({
          osd: { ...(getState().config?.osd ?? cfg.osd), enabled: v },
        }),
      ),
    );

    const hideField = document.createElement("div");
    hideField.className = "field";
    hideField.innerHTML = `
      <div class="field__label">${escapeHtml(t("settings.osdDuration"))}</div>
      <input class="input" type="number" min="500" max="10000" step="100" value="${cfg.osd.autoHideMs}" aria-label="${escapeAttr(t("settings.osdDuration"))}" />
    `;
    hideField.querySelector("input")!.addEventListener("change", (e) => {
      const v = Number((e.target as HTMLInputElement).value);
      void this.queuePatchConfig({
        osd: { ...(getState().config?.osd ?? cfg.osd), autoHideMs: v },
      });
    });
    el.appendChild(hideField);

    const targetField = document.createElement("div");
    targetField.className = "field";
    targetField.innerHTML = `<div class="field__label">${escapeHtml(t("settings.target"))}</div>`;
    const targetSelect = new FluentSelect({
      value: cfg.targetMode,
      options: [
        { value: "mouseMonitor", label: t("settings.target.mouse") },
        { value: "lastUsed", label: t("settings.target.lastUsed") },
        { value: "primary", label: t("settings.target.primary") },
        { value: "fixed", label: t("settings.target.fixed") },
        { value: "allSync", label: t("settings.target.allSync") },
      ],
      ariaLabel: t("settings.target"),
      minWidth: 140,
      onChange: (v) => {
        void this.queuePatchConfig({
          targetMode: v as TargetMode,
        });
      },
    });
    this.selects.push(targetSelect);
    targetField.appendChild(targetSelect.el);
    el.appendChild(targetField);

    el.appendChild(
      rowSwitch(t("settings.syncAll"), cfg.syncAll, (v) =>
        this.queuePatchConfig({ syncAll: v }),
      ),
    );
    el.appendChild(
      rowSwitch(t("settings.rememberLast"), cfg.rememberLastMonitor, (v) =>
        this.queuePatchConfig({ rememberLastMonitor: v }),
      ),
    );
    el.appendChild(
      rowSwitch(t("settings.readOnStart"), cfg.readBrightnessOnStart, (v) =>
        this.queuePatchConfig({ readBrightnessOnStart: v }),
      ),
    );
    el.appendChild(
      rowSwitch(t("settings.showContrast"), cfg.showContrast === true, (v) =>
        this.queuePatchConfig({ showContrast: v }),
      ),
    );
  }

  private fillHotkeys(hotkeys: HotkeyConfig): void {
    const el = this.root.querySelector('[data-sec="hotkeys"]') as HTMLElement;
    for (const action of HOTKEY_ACTIONS) {
      const row = document.createElement("div");
      row.className = "hotkey-row";
      row.innerHTML = `<div class="hotkey-row__name">${escapeHtml(hotkeyLabel(action.id))}</div>`;
      const rec = new HotkeyRecorder({
        value: hotkeys[action.id],
        onRecordingChange: (r) => this.handlers.onRecordingChange(r),
        onChange: async (accel) => {
          const rustAction = hotkeyActionToRust(action.id);
          const saved = await this.handlers.onSetHotkey(rustAction, accel);
          rec.setValue(saved);
        },
      });
      this.recorders.push(rec);
      row.appendChild(rec.el);
      el.appendChild(row);
    }
    const reset = document.createElement("div");
    reset.className = "field";
    reset.innerHTML = `<button type="button" class="text-btn" data-act="reset-hotkeys">${escapeHtml(t("hotkey.reset"))}</button>`;
    reset
      .querySelector("button")!
      .addEventListener("click", () => void this.handlers.onResetHotkeys());
    el.appendChild(reset);
  }

  private fillMonitors(): void {
    const el = this.root.querySelector('[data-sec="monitors"]') as HTMLElement;
    const monitors = getState().monitors;
    if (monitors.length === 0) {
      el.innerHTML = `<div class="field__hint" style="padding:8px 0">${escapeHtml(t("monitors.empty"))}</div>`;
      return;
    }
    for (const m of monitors) {
      const card = document.createElement("div");
      card.className = "monitor-card";
      card.innerHTML = `
        <div class="monitor-card__top">
          <div>
            <div class="monitor-card__name">${escapeHtml(monitorLabel(m))}</div>
            <div class="monitor-card__meta">${escapeHtml(controlMethodLabel(m.controlMethod))} · ${escapeHtml(statusLabel(m.status))} · ${shortId(m.id)}</div>
          </div>
          <div class="field__hint">${m.cachedBrightness}%</div>
        </div>
        <div class="field">
          <div class="field__label">${escapeHtml(t("monitors.alias"))}</div>
          <input class="input" data-alias value="${escapeAttr(m.userAlias ?? "")}" placeholder="${escapeAttr(t("monitors.aliasPlaceholder"))}" aria-label="${escapeAttr(t("monitors.alias"))}" />
        </div>
        <div class="monitor-card__grid">
          <div class="field">
            <div class="field__label">${escapeHtml(t("monitors.sync"))}</div>
            <button type="button" class="switch" data-sync role="switch" aria-checked="${getState().config?.monitorSyncInclude[m.id] !== false}" aria-label="${escapeAttr(t("monitors.sync"))}">
              <span class="switch__knob"></span>
            </button>
          </div>
          <div class="field">
            <div class="field__label">${escapeHtml(t("monitors.fixed"))}</div>
            <button type="button" class="switch" data-fixed role="switch" aria-checked="${getState().config?.fixedMonitorId === m.id}" aria-label="${escapeAttr(t("monitors.fixed"))}">
              <span class="switch__knob"></span>
            </button>
          </div>
        </div>
      `;
      const aliasInput = card.querySelector("[data-alias]") as HTMLInputElement;
      aliasInput.addEventListener("change", () => {
        const v = aliasInput.value.trim();
        void this.handlers.onAlias(m.id, v || null);
      });
      const syncBtn = card.querySelector("[data-sync]") as HTMLButtonElement;
      syncBtn.addEventListener("click", () => {
        const next = syncBtn.getAttribute("aria-checked") !== "true";
        syncBtn.setAttribute("aria-checked", String(next));
        void this.handlers.onSyncInclude(m.id, next);
      });
      const fixedBtn = card.querySelector("[data-fixed]") as HTMLButtonElement;
      fixedBtn.addEventListener("click", () => {
        const next = fixedBtn.getAttribute("aria-checked") !== "true";
        const current = getState().config;
        void this.queuePatchConfig({
          fixedMonitorId: next ? m.id : null,
          targetMode: next ? "fixed" : current?.targetMode,
        });
      });
      el.appendChild(card);
    }
    const refresh = document.createElement("div");
    refresh.className = "field";
    refresh.innerHTML = `<button type="button" class="text-btn" data-act="refresh">${escapeHtml(t("monitors.refresh"))}</button>`;
    refresh
      .querySelector("button")!
      .addEventListener("click", () => void this.handlers.onRefresh());
    el.appendChild(refresh);
  }

  private fillUi(cfg: NonNullable<ReturnType<typeof getState>["config"]>): void {
    const el = this.root.querySelector('[data-sec="ui"]') as HTMLElement;

    const langField = document.createElement("div");
    langField.className = "field";
    langField.innerHTML = `<div class="field__label">${escapeHtml(t("ui.language"))}</div>`;
    const locale = normalizeLocale(cfg.locale);
    const langSelect = new FluentSelect({
      value: locale,
      options: LOCALES.map((l) => ({
        value: l.id,
        label: t(l.labelKey),
      })),
      ariaLabel: t("ui.language"),
      minWidth: 120,
      onChange: (v) => {
        void this.queuePatchConfig({ locale: v });
      },
    });
    this.selects.push(langSelect);
    langField.appendChild(langSelect.el);
    el.appendChild(langField);

    const themeField = document.createElement("div");
    themeField.className = "field";
    themeField.innerHTML = `<div class="field__label">${escapeHtml(t("ui.appearance"))}</div>`;
    const theme = cfg.ui.theme === "light" ? "light" : "dark";
    const themeSelect = new FluentSelect({
      value: theme,
      options: [
        { value: "dark", label: t("ui.theme.dark") },
        { value: "light", label: t("ui.theme.light") },
      ],
      ariaLabel: t("ui.appearance"),
      minWidth: 100,
      onChange: (v) => {
        const current = getState().config;
        void this.queuePatchConfig({
          ui: { ...(current?.ui ?? cfg.ui), theme: v },
        });
      },
    });
    this.selects.push(themeSelect);
    themeField.appendChild(themeSelect.el);
    el.appendChild(themeField);

    el.appendChild(
      rowSwitch(t("ui.animations"), cfg.ui.animations, (v) =>
        this.queuePatchConfig({
          ui: { ...(getState().config?.ui ?? cfg.ui), animations: v },
        }),
      ),
    );
    const op = document.createElement("div");
    op.className = "field";
    op.innerHTML = `
      <div class="field__label">${escapeHtml(t("ui.opacity"))}</div>
      <input class="input" type="number" min="0.5" max="1" step="0.05" value="${cfg.ui.opacity}" aria-label="${escapeAttr(t("ui.opacity"))}" />
    `;
    op.querySelector("input")!.addEventListener("change", (e) => {
      const v = Number((e.target as HTMLInputElement).value);
      void this.queuePatchConfig({
        ui: { ...(getState().config?.ui ?? cfg.ui), opacity: v },
      });
    });
    el.appendChild(op);
  }

  private fillAdvanced(
    cfg: NonNullable<ReturnType<typeof getState>["config"]>,
  ): void {
    const el = this.root.querySelector('[data-sec="advanced"]') as HTMLElement;
    const logField = document.createElement("div");
    logField.className = "field";
    logField.innerHTML = `<div class="field__label">${escapeHtml(t("advanced.logLevel"))}</div>`;
    const logSelect = new FluentSelect({
      value: cfg.logLevel,
      options: [
        { value: "error", label: "Error" },
        { value: "warn", label: "Warn" },
        { value: "info", label: "Info" },
        { value: "debug", label: "Debug" },
      ],
      ariaLabel: t("advanced.logLevel"),
      minWidth: 100,
      onChange: (v) => {
        void this.queuePatchConfig({ logLevel: v });
      },
    });
    this.selects.push(logSelect);
    logField.appendChild(logSelect.el);
    el.appendChild(logField);

    const open = document.createElement("div");
    open.className = "field";
    open.innerHTML = `<button type="button" class="text-btn" data-act="logs">${escapeHtml(t("advanced.openLogs"))}</button>`;
    open
      .querySelector("button")!
      .addEventListener("click", () => void this.handlers.onOpenLogs());
    el.appendChild(open);
  }

  private fillAbout(): void {
    const el = this.root.querySelector('[data-sec="about"]') as HTMLElement;

    // 一行：左 GitHub（可点打开），右「检查更新」
    const row = document.createElement("div");
    row.className = "about-row";

    const gh = document.createElement("button");
    gh.type = "button";
    gh.className = "about-github";
    gh.setAttribute("aria-label", t("about.openRepo"));
    gh.innerHTML = `
      <span class="about-github__icon" aria-hidden="true">${iconGithub}</span>
      <span class="about-github__meta">
        <span class="about-github__label">${escapeHtml(t("about.github"))}</span>
        <span class="about-github__url">${escapeHtml(GITHUB_DISPLAY)}</span>
      </span>
    `;
    gh.addEventListener("click", () => {
      void cmd.openExternalUrl(GITHUB_URL).catch((e) => {
        this.handlers.onToast?.(e instanceof Error ? e.message : String(e));
      });
    });

    const actions = document.createElement("div");
    actions.className = "about-row__actions";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "text-btn";
    btn.textContent = t("about.checkUpdate");
    const openRel = document.createElement("button");
    openRel.type = "button";
    openRel.className = "text-btn";
    openRel.textContent = t("about.openRelease");
    openRel.hidden = true;
    actions.appendChild(btn);
    actions.appendChild(openRel);

    row.appendChild(gh);
    row.appendChild(actions);
    el.appendChild(row);

    // 状态单独一行（检查后才显示）
    const status = document.createElement("div");
    status.className = "about-update-status";
    status.hidden = true;
    el.appendChild(status);

    let releaseUrl = "https://github.com/Elnyxn/Lumaris/releases";
    openRel.addEventListener("click", () => {
      void cmd.openExternalUrl(releaseUrl).catch((e) => {
        this.handlers.onToast?.(e instanceof Error ? e.message : String(e));
      });
    });

    btn.addEventListener("click", async () => {
      btn.disabled = true;
      status.hidden = false;
      status.className = "about-update-status";
      status.textContent = t("about.checking");
      openRel.hidden = true;
      try {
        const r = await cmd.checkForUpdates(true);
        releaseUrl = r.releaseUrl || releaseUrl;
        if (r.error && !r.updateAvailable) {
          status.className = "about-update-status is-err";
          status.textContent = t("about.checkFailed", { err: r.error });
          openRel.hidden = false;
        } else if (r.updateAvailable) {
          status.className = "about-update-status is-new";
          status.textContent = t("about.updateAvailable", {
            latest: r.latestVersion,
            current: r.currentVersion,
          });
          openRel.hidden = false;
        } else {
          status.className = "about-update-status is-ok";
          status.textContent = t("about.upToDate", { v: r.currentVersion });
        }
      } catch (e) {
        status.className = "about-update-status is-err";
        status.textContent = t("about.checkFailed", {
          err: e instanceof Error ? e.message : String(e),
        });
      } finally {
        btn.disabled = false;
      }
    });
  }

  destroyRecorders(): void {
    for (const r of this.recorders) r.destroy();
    this.recorders = [];
  }

  destroySelects(): void {
    for (const s of this.selects) s.destroy();
    this.selects = [];
  }

  destroy(): void {
    this.destroyRecorders();
    this.destroySelects();
  }
}

function rowSwitch(
  label: string,
  checked: boolean,
  onChange: (v: boolean) => void | Promise<void>,
): HTMLElement {
  const el = document.createElement("div");
  el.className = "field";
  el.innerHTML = `
    <div class="field__label">${escapeHtml(label)}</div>
    <button type="button" class="switch" role="switch" aria-checked="${checked}" aria-label="${escapeAttr(label)}">
      <span class="switch__knob"></span>
    </button>
  `;
  const btn = el.querySelector("button")!;
  btn.addEventListener("click", () => {
    const next = btn.getAttribute("aria-checked") !== "true";
    btn.setAttribute("aria-checked", String(next));
    void onChange(next);
  });
  return el;
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
