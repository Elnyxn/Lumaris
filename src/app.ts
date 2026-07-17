import { setLocale, t } from "./i18n";
import * as cmd from "./ipc/commands";
import * as ev from "./ipc/events";
import {
  getState,
  patch,
  setMonitors,
  showToast,
  subscribe,
  updateMonitorBrightness,
  updateMonitorContrast,
} from "./state/store";
import type { PageId } from "./state/types";
import { FlyoutView } from "./ui/flyout";
import { SettingsView } from "./ui/settings";
import { renderToast } from "./ui/toast";

export class App {
  private host: HTMLElement;
  private panel: HTMLElement;
  private flyout: FlyoutView;
  private settings: SettingsView;
  private hideTimer: number | null = null;
  private animating = false;
  private pointerInside = false;
  private recordingHotkey = false;
  private unsubs: Array<() => void> = [];
  private lastRenderKey = "";
  private suppressFullRenderDepth = 0;
  private dragging = false;
  private blurTimer: number | null = null;
  private destroyed = false;

  private readonly onPanelPointerEnter = (): void => {
    this.pointerInside = true;
    this.clearHideTimer();
  };

  private readonly onPanelPointerLeave = (): void => {
    this.pointerInside = false;
    this.resetHideTimer();
  };

  private readonly onWindowKeydown = (e: KeyboardEvent): void => {
    if (e.key === "Escape" && !this.recordingHotkey) {
      void this.hide();
    }
  };

  private readonly onWindowBlur = (): void => {
    if (this.recordingHotkey || this.dragging || this.animating) return;
    if (this.blurTimer !== null) window.clearTimeout(this.blurTimer);
    this.blurTimer = window.setTimeout(() => {
      this.blurTimer = null;
      if (
        !this.destroyed &&
        !document.hasFocus() &&
        getState().visible &&
        !this.recordingHotkey &&
        !this.dragging
      ) {
        void this.hide();
      }
    }, 80);
  };

  constructor(host: HTMLElement) {
    this.host = host;
    this.panel = document.createElement("div");
    // solid 更像系统 Flyout，避免半透明“网页框”与点击不稳
    this.panel.className = "panel fallback-solid";
    this.panel.setAttribute("role", "dialog");
    this.panel.setAttribute("aria-label", "Lumaris");
    this.host.appendChild(this.panel);

    this.flyout = new FlyoutView({
      onBrightness: (id, value, finalWrite) => {
        updateMonitorBrightness(id, value, "cached", true);
        void cmd.setBrightness(id, value, finalWrite);
        if (finalWrite) this.resetHideTimer();
        else this.clearHideTimer();
      },
      onContrast: (id, value, finalWrite) => {
        updateMonitorContrast(id, value, true);
        void cmd.setContrast(id, value, finalWrite);
        if (finalWrite) this.resetHideTimer();
        else this.clearHideTimer();
      },
      onPrev: () => {
        void cmd.selectPrevMonitor().then(() => cmd.getMonitors().then(setMonitors));
        this.resetHideTimer();
      },
      onNext: () => {
        void cmd.selectNextMonitor().then(() => cmd.getMonitors().then(setMonitors));
        this.resetHideTimer();
      },
      onSettings: () => void this.openPage("settings"),
      onSelect: (id) => {
        void cmd.selectMonitor(id).then(() => cmd.getMonitors().then(setMonitors));
      },
      onInteract: () => this.resetHideTimer(),
      onDragState: (d) => {
        this.dragging = d;
        if (d) this.clearHideTimer();
      },
    });

    this.settings = new SettingsView({
      onBack: () => void this.openPage("flyout"),
      onPatchConfig: async (p) => {
        const prevShowContrast = getState().config?.showContrast === true;
        const prevLocale = getState().config?.locale;
        const cfg = await cmd.updateConfig(p);
        // 设置页内控件已本地更新；silent 写 store，避免整页 render 滚回顶部
        // 语言切换需要完整重绘
        const localeChanged = prevLocale !== cfg.locale;
        if (localeChanged) patch({ config: cfg });
        else this.withSuppressedRender(() => patch({ config: cfg }));
        this.applyUiTokens();
        if (localeChanged && getState().page === "settings") {
          this.renderSettingsPreservingScroll();
        }
        this.lastRenderKey = this.renderKey();
        // 开关对比度后需调整浮窗高度
        if (
          getState().page === "settings" &&
          prevShowContrast !== (cfg.showContrast === true)
        ) {
          void cmd.showFlyout("settings");
        }
      },
      onSetHotkey: async (action, accel) => {
        try {
          const saved = await cmd.setHotkey(action, accel);
          const snap = await cmd.getAppSnapshot();
          this.withSuppressedRender(() => patch({ config: snap.config }));
          this.lastRenderKey = this.renderKey();
          showToast(t("hotkey.saved"));
          return saved;
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          showToast(msg);
          throw e;
        }
      },
      onResetHotkeys: async () => {
        const hk = await cmd.resetHotkeys();
        const cfg = getState().config;
        if (cfg) {
          this.withSuppressedRender(() =>
            patch({ config: { ...cfg, hotkeys: hk } }),
          );
        }
        // 快捷键列表需刷新显示，保留滚动位置
        this.renderSettingsPreservingScroll();
        showToast(t("hotkey.resetDone"));
      },
      onSetAutostart: async (enabled) => {
        try {
          const actual = await cmd.setAutostart(enabled);
          this.withSuppressedRender(() => patch({ autostartEnabled: actual }));
          this.lastRenderKey = this.renderKey();
        } catch (e) {
          showToast(e instanceof Error ? e.message : String(e));
          this.renderSettingsPreservingScroll();
        }
      },
      onAlias: async (id, alias) => {
        await cmd.setMonitorAlias(id, alias);
        const monitors = await cmd.getMonitors();
        this.withSuppressedRender(() => setMonitors(monitors));
        this.lastRenderKey = this.renderKey();
      },
      onSyncInclude: async (id, include) => {
        await cmd.setMonitorSyncInclude(id, include);
      },
      onOpenLogs: async () => {
        try {
          await cmd.openLogsDir();
        } catch (e) {
          showToast(e instanceof Error ? e.message : String(e));
        }
      },
      onResetAll: async () => {
        const cfg = await cmd.resetSettings();
        const autostartEnabled = await cmd.getAutostart();
        this.withSuppressedRender(() =>
          patch({
            config: cfg,
            autostartEnabled,
          }),
        );
        this.applyUiTokens();
        this.renderSettingsPreservingScroll();
        showToast(t("toast.resetDone"));
      },
      onRefresh: async () => {
        showToast(t("monitors.refreshing"));
        try {
          await cmd.refreshMonitors();
          // 轮询等待 worker 完成枚举（「仅第二屏幕」切换后可能稍慢）
          let list = getState().monitors;
          for (let i = 0; i < 12; i++) {
            await new Promise((r) => window.setTimeout(r, 120));
            list = await cmd.getMonitors();
            if (list.length > 0) break;
          }
          this.withSuppressedRender(() => setMonitors(list));
          this.lastRenderKey = this.renderKey();
          if (getState().page === "settings") {
            this.renderSettingsPreservingScroll();
          } else {
            this.render();
          }
          // 刷新后按新台数重定位/定高
          await cmd.showFlyout(getState().page || "flyout");
          showToast(t("monitors.refreshed", { n: list.length }));
        } catch (e) {
          showToast(e instanceof Error ? e.message : String(e));
        }
      },
      onRecordingChange: (r) => {
        this.recordingHotkey = r;
        if (r) this.clearHideTimer();
        else this.resetHideTimer();
      },
    });
  }

  /** 设置页重绘时保留滚动位置 */
  private renderSettingsPreservingScroll(): void {
    if (getState().page !== "settings") {
      this.render();
      return;
    }
    const body = this.panel.querySelector(".settings__body") as HTMLElement | null;
    const top = body?.scrollTop ?? 0;
    this.render();
    const body2 = this.panel.querySelector(".settings__body") as HTMLElement | null;
    if (body2) body2.scrollTop = top;
  }

  async start(): Promise<void> {
    if (this.destroyed) return;
    this.unsubs.push(subscribe(() => this.onStoreChange()));

    // 事件
    this.unsubs.push(
      await ev.onMonitorsChanged((m) => {
        if (this.dragging) return;
        setMonitors(m);
      }),
    );
    this.unsubs.push(
      await ev.onBrightnessChanged((e) => {
        if (this.dragging || this.flyout.isDragging()) return;
        // 只改 store + 滑块 DOM，绝不整页 render（否则滑块从旧值重建再跳到新值）
        this.withSuppressedRender(() =>
          updateMonitorBrightness(e.monitorId, e.brightness, e.status, true),
        );
        // 浮窗尚未挂载时（即将 quiet show）只写 store，首绘直接用新值
        if (this.panel.querySelector(".flyout")) {
          this.flyout.updateBrightnessVisual(e.monitorId, e.brightness);
        }
        // 调节中续命 OSD，不触发 show/focus
        if (getState().visible) this.resetHideTimer();
      }),
    );
    this.unsubs.push(
      await ev.onContrastChanged((e) => {
        if (this.dragging || this.flyout.isDragging()) return;
        this.withSuppressedRender(() =>
          updateMonitorContrast(e.monitorId, e.contrast, true),
        );
        this.flyout.updateContrastVisual(e.monitorId, e.contrast);
      }),
    );
    this.unsubs.push(
      await ev.onOperationResult((e) => {
        if (!e.success && e.error) showToast(e.error);
      }),
    );
    this.unsubs.push(
      await ev.onAppError((e) => showToast(e.message)),
    );
    this.unsubs.push(
      await ev.onSettingsChanged((c) => {
        // 设置页调整中：只同步 store，不触发整页重绘
        this.withSuppressedRender(() => patch({ config: c }));
        this.applyUiTokens();
        this.lastRenderKey = this.renderKey();
      }),
    );
    this.unsubs.push(
      await ev.onAppState((s) => {
        patch({
          config: s.config,
          monitors: s.monitors,
          autostartEnabled: s.autostartEnabled,
          version: s.version,
          ready: true,
        });
      }),
    );
    this.unsubs.push(
      await ev.onUiShowFlyout((page, quiet, brightness) => {
        const p = ((page as PageId) || "flyout") as PageId;
        // quiet / 浮窗：轻量 OSD，禁止整页重建与抢焦点
        if (quiet || p === "flyout") {
          void this.showOsd(p, brightness);
        } else {
          void this.openPage(p, true);
        }
      }),
    );
    this.unsubs.push(
      await ev.onUiToggleFlyout(() => {
        void this.toggle();
      }),
    );
    this.unsubs.push(
      await ev.onWindowHidden(() => {
        patch({ visible: false });
        this.clearHideTimer();
      }),
    );

    // 交互：自动隐藏
    this.panel.addEventListener("pointerenter", this.onPanelPointerEnter);
    this.panel.addEventListener("pointerleave", this.onPanelPointerLeave);

    window.addEventListener("keydown", this.onWindowKeydown);

    // 失焦兜底：点击窗外时隐藏（与后端 Focused(false) 双保险）
    // 快捷键录制中不隐藏
    window.addEventListener("blur", this.onWindowBlur);

    try {
      await cmd.frontendReady();
      const snap = await cmd.getAppSnapshot();
      patch({
        config: snap.config,
        monitors: snap.monitors,
        autostartEnabled: snap.autostartEnabled,
        version: snap.version,
        ready: true,
      });
      this.applyUiTokens();
      this.render();
      this.host.classList.add("ready");
    } catch (e) {
      console.error(e);
      await cmd.reportUiError(String(e), "init");
      this.host.classList.add("ready");
      showToast(t("initFailed"));
    }
  }

  private onStoreChange(): void {
    renderToast(this.host, getState().toast);
    // 同步语言（设置里切换后立刻生效）
    const loc = getState().config?.locale;
    if (loc) setLocale(loc);
    if (this.suppressFullRenderDepth > 0 || this.dragging) return;
    if (!getState().visible && !getState().ready) return;
    // 设置页内改开关/下拉：禁止整页 render（会滚回顶部）
    // 但语言切换需要整页重绘
    if (getState().page === "settings") {
      const key = this.renderKey();
      if (key !== this.lastRenderKey) {
        // locale / monitors 变化时刷新设置页
        if (key.split("|")[0] !== this.lastRenderKey.split("|")[0] || key.includes("loc:")) {
          this.renderSettingsPreservingScroll();
        }
        this.lastRenderKey = key;
      }
      return;
    }
    const key = this.renderKey();
    if (key === this.lastRenderKey && getState().visible) {
      return;
    }
    if (getState().visible || getState().ready) {
      this.render();
    }
  }

  private withSuppressedRender<T>(fn: () => T): T {
    this.suppressFullRenderDepth += 1;
    try {
      return fn();
    } finally {
      this.suppressFullRenderDepth = Math.max(
        0,
        this.suppressFullRenderDepth - 1,
      );
    }
  }

  private renderKey(): string {
    const st = getState();
    // 设置项不进 key，避免改步长/开关触发重绘；locale 进 key 以便切语言
    const mids = st.monitors
      .map(
        (m) =>
          `${m.id}:${m.isSelected}:${m.status}:${m.userAlias ?? ""}:${m.isControllable}`,
      )
      .join("|");
    return `${st.page}|loc:${st.config?.locale ?? "zh-CN"}|${st.config?.layoutMode}|${mids}`;
  }

  private render(): void {
    const st = getState();
    this.lastRenderKey = this.renderKey();
    // 设置控件把菜单挂到 body、录制器挂到 window，切页前必须显式释放。
    this.settings.destroy();
    this.panel.innerHTML = "";
    if (st.page === "settings") {
      this.settings.render();
      this.panel.appendChild(this.settings.root);
    } else {
      this.flyout.render();
      this.panel.appendChild(this.flyout.root);
    }
  }

  private applyUiTokens(): void {
    const cfg = getState().config;
    if (!cfg) return;
    if (cfg.locale) setLocale(cfg.locale);
    const ui = cfg.ui;
    if (!ui) return;
    document.documentElement.style.setProperty(
      "--opacity-panel",
      String(ui.opacity),
    );
    const theme = ui.theme === "light" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", theme);
    document.body?.setAttribute("data-theme", theme);
  }

  private async openPage(page: PageId, forceShow = false): Promise<void> {
    const prev = getState().page;
    const fromSettings = prev === "settings" && page === "flyout";
    const toSettings = prev === "flyout" && page === "settings";

    patch({ page });
    this.panel.classList.remove("page-from-settings", "page-to-settings");
    this.render();

    // 设置返回浮窗 / 进入设置：内容区轻量滑动，不带动窗口位移
    if (fromSettings) {
      this.panel.classList.add("page-from-settings");
      window.setTimeout(() => {
        this.panel.classList.remove("page-from-settings");
      }, 200);
    } else if (toSettings) {
      this.panel.classList.add("page-to-settings");
      window.setTimeout(() => {
        this.panel.classList.remove("page-to-settings");
      }, 200);
    }

    if (forceShow || !getState().visible) {
      await this.show();
    } else {
      await cmd.showFlyout(page);
      if (page === "settings") this.clearHideTimer();
      else this.resetHideTimer();
    }
  }

  async toggle(): Promise<void> {
    if (getState().visible) await this.hide();
    else await this.openPage("flyout", true);
  }

  async show(): Promise<void> {
    if (this.animating) return;
    const page = getState().page || "flyout";

    // 内容先就绪（窗口仍可隐藏），禁止 show 后再 render 造成组件闪动
    if (!this.panel.querySelector(".flyout") && !this.panel.querySelector(".settings")) {
      this.render();
    } else if (getState().page !== page) {
      this.render();
    }

    // 无入场位移动画：只定位一次再显示
    await cmd.showFlyout(page);
    patch({ visible: true });
    // 不再二次 render / 不再 anim-show

    if (page === "settings") this.clearHideTimer();
    else this.resetHideTimer();
  }

  /** 合并并发 OSD 请求，避免连发快捷键叠多次 show */
  private osdBusy = false;
  private osdPending: { page: PageId; snaps?: { id: string; brightness: number }[] } | null =
    null;

  /**
   * 快捷键/托盘滚轮 OSD：
   * - 先把亮度写入 store，再决定是否首绘
   * - 已有浮窗只改滑块，不 innerHTML 重建
   * - 再 show 窗口（已显示则后端 no-op，不抢焦点）
   */
  private async showOsd(
    page: PageId = "flyout",
    snaps?: { id: string; brightness: number }[],
  ): Promise<void> {
    if (page === "settings") {
      await this.openPage("settings", true);
      return;
    }

    // 合并连发
    if (this.osdBusy) {
      this.osdPending = { page: "flyout", snaps };
      if (snaps) {
        this.withSuppressedRender(() => {
          for (const s of snaps) {
            updateMonitorBrightness(s.id, s.brightness, undefined, true);
          }
        });
        if (this.panel.querySelector(".flyout")) {
          for (const s of snaps) {
            this.flyout.updateBrightnessVisual(s.id, s.brightness);
          }
        }
      }
      this.resetHideTimer();
      return;
    }
    this.osdBusy = true;
    try {
      // 1) 先应用随事件带来的亮度快照，首绘就用新值
      if (snaps?.length) {
        this.withSuppressedRender(() => {
          for (const s of snaps) {
            updateMonitorBrightness(s.id, s.brightness, undefined, true);
          }
        });
      }

      if (getState().page !== "flyout") {
        this.withSuppressedRender(() => patch({ page: "flyout" }));
      }

      const hadFlyout = !!this.panel.querySelector(".flyout");
      if (!hadFlyout) {
        this.render();
      } else {
        for (const m of getState().monitors) {
          this.flyout.updateBrightnessVisual(m.id, m.cachedBrightness);
        }
      }

      // 2) 内容就绪后再 show（已显示则 commands 直接 return）
      await cmd.showFlyout("flyout");
      this.withSuppressedRender(() => patch({ visible: true }));
      this.lastRenderKey = this.renderKey();
      this.resetHideTimer();
    } finally {
      this.osdBusy = false;
      const pending = this.osdPending;
      this.osdPending = null;
      if (pending) {
        void this.showOsd(pending.page, pending.snaps);
      }
    }
  }

  async hide(): Promise<void> {
    if (!getState().visible || this.animating) return;
    this.clearHideTimer();
    // 立即隐藏，无退场动画（避免残影和位移）
    await cmd.hideFlyout();
    patch({ visible: false, page: "flyout" });
    this.animating = false;
  }

  private resetHideTimer(): void {
    this.clearHideTimer();
    const st = getState();
    if (!st.visible || st.page === "settings" || this.recordingHotkey) return;
    if (this.pointerInside) return;
    if (st.config?.osd.enabled === false) return;
    const ms = st.config?.osd.autoHideMs ?? 1800;
    this.hideTimer = window.setTimeout(() => {
      void this.hide();
    }, ms);
  }

  private clearHideTimer(): void {
    if (this.hideTimer !== null) {
      window.clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.clearHideTimer();
    if (this.blurTimer !== null) {
      window.clearTimeout(this.blurTimer);
      this.blurTimer = null;
    }
    this.panel.removeEventListener("pointerenter", this.onPanelPointerEnter);
    this.panel.removeEventListener("pointerleave", this.onPanelPointerLeave);
    window.removeEventListener("keydown", this.onWindowKeydown);
    window.removeEventListener("blur", this.onWindowBlur);
    for (const unsubscribe of this.unsubs.splice(0)) unsubscribe();
    this.settings.destroy();
    this.panel.replaceChildren();
    this.panel.remove();
  }
}
