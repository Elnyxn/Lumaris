/** Fluent 风格自定义下拉（替代原生 select 白底弹层） */

export interface SelectOption {
  value: string;
  label: string;
}

export interface FluentSelectOptions {
  value: string;
  options: SelectOption[];
  ariaLabel?: string;
  onChange: (value: string) => void;
  minWidth?: number;
}

let openSelect: FluentSelect | null = null;

export class FluentSelect {
  readonly el: HTMLElement;
  private btn: HTMLButtonElement;
  private menu: HTMLElement;
  private value: string;
  private options: SelectOption[];
  private onChange: (value: string) => void;
  private open = false;
  private destroyed = false;
  private activationTimer: number | null = null;
  private onDocPointer: ((e: PointerEvent) => void) | null = null;
  private onScroll: (() => void) | null = null;

  constructor(opts: FluentSelectOptions) {
    this.value = opts.value;
    this.options = opts.options;
    this.onChange = opts.onChange;

    this.el = document.createElement("div");
    this.el.className = "fselect";
    if (opts.minWidth) this.el.style.minWidth = `${opts.minWidth}px`;

    this.btn = document.createElement("button");
    this.btn.type = "button";
    this.btn.className = "fselect__btn";
    this.btn.setAttribute("aria-haspopup", "listbox");
    this.btn.setAttribute("aria-expanded", "false");
    if (opts.ariaLabel) this.btn.setAttribute("aria-label", opts.ariaLabel);

    this.menu = document.createElement("div");
    this.menu.className = "fselect__menu";
    this.menu.setAttribute("role", "listbox");
    this.menu.hidden = true;

    this.el.appendChild(this.btn);
    // menu 挂到 body，避免被 settings 滚动容器裁剪
    document.body.appendChild(this.menu);

    this.paintBtn();
    this.paintMenu();

    this.btn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.toggle();
    });
  }

  getValue(): string {
    return this.value;
  }

  setValue(v: string): void {
    this.value = v;
    this.paintBtn();
    this.paintMenu();
  }

  private labelOf(v: string): string {
    return this.options.find((o) => o.value === v)?.label ?? v;
  }

  private paintBtn(): void {
    this.btn.innerHTML = `
      <span class="fselect__text">${escapeHtml(this.labelOf(this.value))}</span>
      <span class="fselect__chev" aria-hidden="true">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M6 9l6 6 6-6"/>
        </svg>
      </span>
    `;
  }

  private paintMenu(): void {
    this.menu.innerHTML = "";
    for (const o of this.options) {
      const item = document.createElement("button");
      item.type = "button";
      item.className =
        "fselect__item" + (o.value === this.value ? " is-selected" : "");
      item.setAttribute("role", "option");
      item.setAttribute("aria-selected", String(o.value === this.value));
      item.textContent = o.label;
      item.addEventListener("click", (e) => {
        e.stopPropagation();
        this.pick(o.value);
      });
      this.menu.appendChild(item);
    }
  }

  private pick(v: string): void {
    if (v !== this.value) {
      this.value = v;
      this.paintBtn();
      this.paintMenu();
      this.onChange(v);
    }
    this.close();
  }

  toggle(): void {
    if (this.open) this.close();
    else this.show();
  }

  show(): void {
    if (this.destroyed) return;
    if (openSelect && openSelect !== this) openSelect.close();
    openSelect = this;
    this.open = true;
    this.menu.hidden = false;
    this.el.classList.add("is-open");
    this.btn.setAttribute("aria-expanded", "true");
    this.positionMenu();

    this.onDocPointer = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!this.el.contains(t) && !this.menu.contains(t)) this.close();
    };
    this.onScroll = () => this.positionMenu();

    this.activationTimer = window.setTimeout(() => {
      this.activationTimer = null;
      if (!this.open || this.destroyed || !this.onDocPointer || !this.onScroll) {
        return;
      }
      document.addEventListener("pointerdown", this.onDocPointer, true);
      window.addEventListener("scroll", this.onScroll, true);
      window.addEventListener("resize", this.onScroll, true);
    }, 0);
  }

  close(): void {
    if (this.activationTimer !== null) {
      window.clearTimeout(this.activationTimer);
      this.activationTimer = null;
    }
    if (!this.open) return;
    this.open = false;
    this.menu.hidden = true;
    this.el.classList.remove("is-open");
    this.btn.setAttribute("aria-expanded", "false");
    if (this.onDocPointer) {
      document.removeEventListener("pointerdown", this.onDocPointer, true);
      this.onDocPointer = null;
    }
    if (this.onScroll) {
      window.removeEventListener("scroll", this.onScroll, true);
      window.removeEventListener("resize", this.onScroll, true);
      this.onScroll = null;
    }
    if (openSelect === this) openSelect = null;
  }

  private positionMenu(): void {
    const r = this.btn.getBoundingClientRect();
    const menuH = Math.min(200, this.options.length * 34 + 12);
    const spaceBelow = window.innerHeight - r.bottom - 8;
    const openUp = spaceBelow < menuH && r.top > spaceBelow;

    this.menu.style.position = "fixed";
    this.menu.style.left = `${Math.round(r.left)}px`;
    this.menu.style.minWidth = `${Math.round(r.width)}px`;
    this.menu.style.right = "auto";
    this.menu.style.zIndex = "200";

    if (openUp) {
      this.menu.style.top = "auto";
      this.menu.style.bottom = `${Math.round(window.innerHeight - r.top + 4)}px`;
    } else {
      this.menu.style.bottom = "auto";
      this.menu.style.top = `${Math.round(r.bottom + 4)}px`;
    }
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.close();
    this.menu.remove();
  }
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
