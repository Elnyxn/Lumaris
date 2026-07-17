export function ensureToastHost(root: HTMLElement): HTMLElement {
  let el = root.querySelector(".toast") as HTMLElement | null;
  if (!el) {
    el = document.createElement("div");
    el.className = "toast";
    el.hidden = true;
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    root.appendChild(el);
  }
  return el;
}

export function renderToast(root: HTMLElement, message: string | null): void {
  const el = ensureToastHost(root);
  if (!message) {
    el.hidden = true;
    el.textContent = "";
    return;
  }
  el.hidden = false;
  el.textContent = message;
}
