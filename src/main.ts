import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/acrylic.css";
import "./styles/controls.css";
import "./styles/flyout.css";
import "./styles/settings.css";
import { App } from "./app";

function boot(): void {
  const host = document.getElementById("app");
  if (!host) return;
  const app = new App(host);
  void app.start().catch((err) => {
    console.error("Lumaris 启动失败", err);
    host.classList.add("ready");
    host.textContent = "Failed / 启动失败";
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
