import { t } from "../i18n";
import { formatHotkey } from "../utils/formatting";

export interface HotkeyRecorderOptions {
  value: string | null;
  onChange: (accel: string | null) => void | Promise<void>;
  onRecordingChange?: (recording: boolean) => void;
}

let activeRecorder: HotkeyRecorder | null = null;

export class HotkeyRecorder {
  readonly el: HTMLElement;
  private chip: HTMLButtonElement;
  private recording = false;
  private value: string | null;
  private onChange: (accel: string | null) => void | Promise<void>;
  private onRecordingChange?: (recording: boolean) => void;
  private keyHandler: ((e: KeyboardEvent) => void) | null = null;
  private destroyed = false;

  constructor(opts: HotkeyRecorderOptions) {
    this.value = opts.value;
    this.onChange = opts.onChange;
    this.onRecordingChange = opts.onRecordingChange;

    this.el = document.createElement("div");
    this.el.className = "hotkey-row__actions";
    this.el.innerHTML = `
      <button type="button" class="hotkey-chip" aria-label="${t("hotkey.recordAria")}">${formatHotkey(this.value)}</button>
      <button type="button" class="text-btn" data-act="clear" aria-label="${t("hotkey.clearAria")}">${t("hotkey.clear")}</button>
    `;
    this.chip = this.el.querySelector(".hotkey-chip")!;

    this.chip.addEventListener("click", () => this.toggleRecord());
    this.el.querySelector('[data-act="clear"]')!.addEventListener("click", () => {
      this.stopRecord();
      void this.commit(null);
    });
  }

  setValue(v: string | null): void {
    this.value = v;
    if (!this.recording) {
      this.chip.textContent = formatHotkey(v);
      this.chip.classList.remove("is-recording", "has-error");
    }
  }

  setError(msg: string | null): void {
    this.chip.classList.toggle("has-error", !!msg);
    if (msg) this.chip.title = msg;
    else this.chip.removeAttribute("title");
  }

  private toggleRecord(): void {
    if (this.recording) this.stopRecord();
    else this.startRecord();
  }

  private startRecord(): void {
    if (this.destroyed) return;
    if (activeRecorder && activeRecorder !== this) {
      activeRecorder.stopRecord();
    }
    activeRecorder = this;
    this.recording = true;
    this.chip.classList.add("is-recording");
    this.chip.textContent = t("hotkey.press");
    this.onRecordingChange?.(true);
    this.keyHandler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        this.stopRecord();
        this.chip.textContent = formatHotkey(this.value);
        return;
      }
      if (e.repeat) return;
      // 仅修饰键时等待
      if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;

      const parts: string[] = [];
      if (e.ctrlKey) parts.push("Ctrl");
      if (e.altKey) parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");
      if (e.metaKey) parts.push("Super");

      const key = mapKey(e);
      if (!key) return;
      if (parts.length === 0 && !(key.startsWith("F") && key.length <= 3)) {
        this.chip.textContent = t("hotkey.needModifier");
        return;
      }
      parts.push(key);
      const accel = parts.join("+");
      this.stopRecord();
      void this.commit(accel);
    };
    window.addEventListener("keydown", this.keyHandler, true);
  }

  private stopRecord(): void {
    if (!this.recording && !this.keyHandler) {
      if (activeRecorder === this) activeRecorder = null;
      return;
    }
    this.recording = false;
    this.chip.classList.remove("is-recording");
    this.onRecordingChange?.(false);
    if (this.keyHandler) {
      window.removeEventListener("keydown", this.keyHandler, true);
      this.keyHandler = null;
    }
    if (activeRecorder === this) activeRecorder = null;
  }

  private async commit(accel: string | null): Promise<void> {
    if (this.destroyed) return;
    this.chip.textContent = formatHotkey(accel);
    try {
      await this.onChange(accel);
      if (this.destroyed) return;
      this.value = accel;
      this.setError(null);
    } catch (err) {
      if (this.destroyed) return;
      const msg = err instanceof Error ? err.message : String(err);
      this.setError(msg);
      this.chip.textContent = formatHotkey(this.value);
    }
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.stopRecord();
  }
}

function mapKey(e: KeyboardEvent): string | null {
  const k = e.key;
  if (k === " ") return "Space";
  if (k === "ArrowUp") return "ArrowUp";
  if (k === "ArrowDown") return "ArrowDown";
  if (k === "ArrowLeft") return "ArrowLeft";
  if (k === "ArrowRight") return "ArrowRight";
  if (k === "PageUp") return "PageUp";
  if (k === "PageDown") return "PageDown";
  if (k === "Home") return "Home";
  if (k === "End") return "End";
  if (k === "Insert") return "Insert";
  if (k === "Delete") return "Delete";
  if (k === "+" || k === "=") return "Plus";
  if (k === "-" || k === "_") return "Minus";
  if (/^F\d{1,2}$/i.test(k)) return k.toUpperCase();
  if (k.length === 1 && /[a-zA-Z0-9]/.test(k)) return k.toUpperCase();
  if (e.code.startsWith("Numpad")) return e.code;
  return null;
}
