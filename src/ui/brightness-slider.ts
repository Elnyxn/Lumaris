/**
 * 丝滑亮度滑块
 * - 拖动/滚轮：本地 1% 跟手，不吃外部回写
 * - 滚轮：过程中不发 IPC，停轮后再 final 一次（杜绝回缩）
 * - 拖动：节流 IPC；松手 final
 */

export interface SliderOptions {
  min?: number;
  max?: number;
  step?: number;
  value?: number;
  disabled?: boolean;
  ariaLabel?: string;
  onInput: (value: number, finalWrite: boolean) => void;
  /** 仅刷新数字/UI，不写硬件 */
  onPreview?: (value: number) => void;
  /** 交互中（拖动或滚轮进行中） */
  onDragState?: (active: boolean) => void;
}

export class BrightnessSlider {
  readonly el: HTMLElement;
  private trackFill: HTMLElement;
  private thumb: HTMLElement;
  private value = 0;
  private min = 0;
  private max = 100;
  private step = 5;
  private disabled = false;
  /** 指针拖动 */
  private pointerDown = false;
  /** 滚轮会话中 */
  private wheelActive = false;
  private onInput: (value: number, finalWrite: boolean) => void;
  private onPreview?: (value: number) => void;
  private onDragState?: (active: boolean) => void;
  private ipcTimer: number | null = null;
  private lastIpcAt = 0;
  private pendingIpc: number | null = null;
  private wheelTimer: number | null = null;
  /** 本地权威值，最终提交后短时拒绝外部回写 */
  private lastUserValue: number | null = null;
  private userHoldUntil = 0;
  private activeState = false;

  constructor(opts: SliderOptions) {
    this.min = opts.min ?? 0;
    this.max = opts.max ?? 100;
    this.step = Math.max(1, opts.step ?? 5);
    this.value = this.clamp(opts.value ?? 0);
    this.disabled = opts.disabled ?? false;
    this.onInput = opts.onInput;
    this.onPreview = opts.onPreview;
    this.onDragState = opts.onDragState;

    this.el = document.createElement("div");
    this.el.className = "slider";
    this.el.setAttribute("role", "slider");
    this.el.setAttribute("aria-valuemin", String(this.min));
    this.el.setAttribute("aria-valuemax", String(this.max));
    this.el.setAttribute("aria-label", opts.ariaLabel ?? "亮度");
    this.el.tabIndex = 0;
    this.el.innerHTML = `
      <div class="slider__track"><div class="slider__fill"></div></div>
      <div class="slider__thumb"></div>
    `;
    this.trackFill = this.el.querySelector(".slider__fill")!;
    this.thumb = this.el.querySelector(".slider__thumb")!;
    this.bind();
    this.paint();
    if (this.disabled) this.setDisabled(true);
  }

  /** 拖动或滚轮进行中 */
  isDragging(): boolean {
    return this.pointerDown || this.wheelActive;
  }

  private clamp(v: number): number {
    return Math.min(this.max, Math.max(this.min, Math.round(v)));
  }

  private syncActiveState(): void {
    const active = this.isDragging();
    this.el.classList.toggle("is-dragging", active);
    if (active !== this.activeState) {
      this.activeState = active;
      this.onDragState?.(active);
    }
  }

  private bind(): void {
    this.el.addEventListener("pointerdown", (e) => {
      if (this.disabled) return;
      e.preventDefault();
      this.pointerDown = true;
      this.syncActiveState();
      try {
        this.el.setPointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      this.applyFromPointer(e);
    });

    this.el.addEventListener("pointermove", (e) => {
      if (!this.pointerDown) return;
      e.preventDefault();
      this.applyFromPointer(e);
    });

    const endPointer = (e: PointerEvent) => {
      if (!this.pointerDown) return;
      e.preventDefault();
      this.pointerDown = false;
      try {
        this.el.releasePointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      this.commitFinal();
      this.syncActiveState();
    };

    this.el.addEventListener("pointerup", endPointer);
    this.el.addEventListener("pointercancel", endPointer);
    this.el.addEventListener("lostpointercapture", () => {
      if (!this.pointerDown) return;
      this.pointerDown = false;
      this.commitFinal();
      this.syncActiveState();
    });

    this.el.addEventListener("keydown", (e) => {
      if (this.disabled) return;
      let next = this.value;
      switch (e.key) {
        case "ArrowLeft":
        case "ArrowDown":
          next = this.value - this.step;
          break;
        case "ArrowRight":
        case "ArrowUp":
          next = this.value + this.step;
          break;
        case "PageDown":
          next = this.value - this.step * 2;
          break;
        case "PageUp":
          next = this.value + this.step * 2;
          break;
        case "Home":
          next = this.min;
          break;
        case "End":
          next = this.max;
          break;
        default:
          return;
      }
      e.preventDefault();
      this.value = this.clamp(next);
      this.paint();
      this.onPreview?.(this.value);
      this.commitFinal();
    });

    // 滚轮：数字立刻变；硬件仍停轮后 final 一次
    this.el.addEventListener(
      "wheel",
      (e) => {
        if (this.disabled) return;
        e.preventDefault();
        e.stopPropagation();

        if (!this.wheelActive) {
          this.wheelActive = true;
          this.syncActiveState();
        }

        // 使用设置里的亮度步长（快捷键/滚轮共用），一次滚轮一档
        const dir = e.deltaY > 0 ? -1 : 1;
        const next = this.clamp(this.value + dir * this.step);
        if (next !== this.value) {
          this.value = next;
          this.paint();
          this.onPreview?.(next); // 百分比立刻更新
          this.pendingIpc = next;
        }

        if (this.wheelTimer !== null) window.clearTimeout(this.wheelTimer);
        this.wheelTimer = window.setTimeout(() => {
          this.wheelTimer = null;
          this.wheelActive = false;
          this.commitFinal();
          this.syncActiveState();
        }, 160);
      },
      { passive: false },
    );
  }

  private applyFromPointer(e: PointerEvent): void {
    const rect = this.el.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    const v = this.clamp(this.min + ratio * (this.max - this.min));
    if (v === this.value) return;
    this.value = v;
    this.paint();
    this.onPreview?.(v);
    this.queueThrottledIpc(v);
  }

  /** 拖动中节流写硬件；滚轮不用 */
  private queueThrottledIpc(v: number): void {
    this.pendingIpc = v;
    const now = performance.now();
    const wait = 100 - (now - this.lastIpcAt);
    if (wait <= 0) {
      this.flushIpc(false);
    } else if (this.ipcTimer === null) {
      this.ipcTimer = window.setTimeout(() => {
        this.ipcTimer = null;
        this.flushIpc(false);
      }, wait);
    }
  }

  private flushIpc(final: boolean): void {
    if (this.ipcTimer !== null) {
      window.clearTimeout(this.ipcTimer);
      this.ipcTimer = null;
    }
    const v = this.pendingIpc ?? this.value;
    this.pendingIpc = null;
    this.lastIpcAt = performance.now();
    if (final) {
      this.lastUserValue = v;
      this.userHoldUntil = performance.now() + 600;
    }
    this.onInput(v, final);
  }

  private commitFinal(): void {
    this.pendingIpc = this.value;
    this.flushIpc(true);
  }

  /**
   * 外部同步。交互中 / 用户刚提交后的 hold 窗口内忽略，防回缩。
   * 值未变则完全不 paint，避免快捷键连发时无意义布局抖动。
   */
  setValue(v: number, emitFinal = false, force = false): void {
    if (!force) {
      if (this.isDragging()) return;
      if (
        this.lastUserValue !== null &&
        performance.now() < this.userHoldUntil &&
        Math.abs(this.clamp(v) - this.lastUserValue) <= 2
      ) {
        // 硬件回读与用户值接近时，保持用户值，不弹
        return;
      }
      if (
        this.lastUserValue !== null &&
        performance.now() < this.userHoldUntil
      ) {
        // hold 期内一律以用户为准
        return;
      }
    }
    const next = this.clamp(v);
    if (next === this.value && !emitFinal) return;
    this.value = next;
    this.paint();
    if (emitFinal) {
      this.pendingIpc = next;
      this.flushIpc(true);
    }
  }

  setStep(step: number): void {
    this.step = Math.max(1, step);
  }

  setDisabled(d: boolean): void {
    this.disabled = d;
    this.el.classList.toggle("is-disabled", d);
    this.el.setAttribute("aria-disabled", String(d));
    this.el.tabIndex = d ? -1 : 0;
  }

  getValue(): number {
    return this.value;
  }

  private paint(): void {
    const pct =
      this.max <= this.min
        ? 0
        : ((this.value - this.min) / (this.max - this.min)) * 100;
    this.trackFill.style.width = `${pct}%`;
    this.thumb.style.left = `${pct}%`;
    this.el.setAttribute("aria-valuenow", String(this.value));
    this.el.setAttribute("aria-valuetext", `${this.value}%`);
  }
}
