<p align="center">
  <img src="docs/assets/logo.png" width="120" height="120" alt="Lumaris logo" />
</p>

<h1 align="center">Lumaris</h1>

<p align="center">
  <strong>Luma × Polaris</strong> — Professional display brightness control for Windows<br/>
  Fluent UI · Tray-resident · External DDC/CI · Laptop WMI backlight · Multi-monitor
</p>

<p align="center">
  <a href="README.md"><strong>简体中文</strong></a>
  ·
  <strong>English</strong>
</p>

<p align="center">
  <a href="#download"><img src="https://img.shields.io/badge/Download-Setup%20%7C%20Portable-0A84FF?style=for-the-badge" alt="Download" /></a>
  <a href="https://github.com/Elnyxn/Lumaris/releases"><img src="https://img.shields.io/github/v/release/Elnyxn/Lumaris?style=for-the-badge&color=6E56CF" alt="Release" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/License-PolyForm%20Noncommercial-F59E0B?style=for-the-badge" alt="License" /></a>
  <a href="#platform"><img src="https://img.shields.io/badge/Platform-Windows%2010%2F11-0078D4?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=black" alt="Tauri" />
  <img src="https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/TypeScript-5-3178C6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/i18n-zh--CN%20%7C%20EN-8B5CF6" alt="i18n" />
</p>

---

## Highlights

| | Feature | Description |
|:---:|:---|:---|
| 🔆 | **Precise brightness** | External displays via DDC/CI (VCP 0x10) / standard brightness API; laptop panels via WMI/ACPI backlight |
| 🖥️ | **Multi-monitor** | Vertical stack layout; sync / independent / fixed targets |
| ⌨️ | **Global hotkeys** | Fully customizable; hold-to-accelerate curve; tap uses configured step |
| 🖱️ | **Tray scroll** | Hover the tray icon and scroll to adjust — no window required |
| 🎨 | **Fluent UI** | Dark / light themes aligned with Windows 11 flyouts |
| 🌐 | **Bilingual** | Full zh-CN / English UI and tray strings |
| ⚡ | **Lightweight** | Single-instance tray app, low footprint, optional autostart |

---

## Screenshots

<p align="center">
  <img src="docs/assets/screenshot-dual.png" width="440" alt="Multi-monitor brightness flyout" /><br/>
  <em>Multi-monitor brightness flyout</em>
</p>

<p align="center">
  <img src="docs/assets/screenshot-flyout.png" width="340" alt="Single display brightness and contrast" />
  &nbsp;
  <img src="docs/assets/screenshot-light.png" width="340" alt="Light theme" /><br/>
  <em>Single display · brightness + contrast · light theme</em>
</p>

<p align="center">
  <img src="docs/assets/screenshot-settings.png" width="360" alt="Settings" /><br/>
  <em>Settings</em>
</p>

---

## Download

Get the latest build from [**GitHub Releases**](https://github.com/Elnyxn/Lumaris/releases):

| Package | File | Best for |
|:---|:---|:---|
| **Installer** | `Lumaris-Setup-x.y.z.exe` | Start Menu, uninstaller, optional autostart |
| **Portable** | `Lumaris-portable-x.y.z.zip` | Unzip and run |

> Requires **Microsoft Edge WebView2 Runtime** (preinstalled on most Windows 10/11 PCs).

Checksums ship as `SHA256SUMS.txt` on the release page.

---

## Getting started

1. Install the Setup package or extract the portable zip  
2. Find the **Lumaris** tray icon  
3. **Click** the tray icon to open the flyout; **hover + scroll** to change brightness  
4. Open settings (gear) for hotkeys, theme, language, monitor aliases  

### Default hotkeys (editable)

| Action | Default |
|:---|:---|
| Increase brightness | `Ctrl + Alt + ↑` |
| Decrease brightness | `Ctrl + Alt + ↓` |
| Toggle flyout | `Ctrl + Alt + B` |
| Previous / next display | `Ctrl + Alt + ← / →` |

---

## How it works

```text
┌──────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ Flyout / keys│ ──► │ Target resolve   │ ──► │ DDC worker      │
│ Tray scroll  │     │ Optimistic UI    │     │ Serialized I/O  │
└──────────────┘     └──────────────────┘     └────────┬────────┘
                                                       │
                     ┌──────────────────┐              │
                     │ External DDC/CI  │ ◄────────────┤
                     │ Laptop WMI BL    │ ◄────────────┘
                     └──────────────────┘
```

- **External monitors**: `SetMonitorBrightness` / VCP `0x10`  
- **Laptop panel**: `root\wmi` → `WmiMonitorBrightness` / `WmiSetBrightness`, matched by hardware InstanceName so “Second screen only” does not mis-bind externals  

---

## Tech stack

| Layer | Stack |
|:---|:---|
| Shell | [Tauri 2](https://tauri.app/) |
| OS / hardware | Rust · windows-rs · Win32 Monitor Configuration API · WMI |
| UI | TypeScript · Vite · native HTML/CSS (no heavy SPA framework) |
| Runtime | Single WebView2 instance |

---

## Build from source

### Requirements

- Windows 10 **1809+** / Windows 11  
- [Rust](https://rustup.rs/) 1.77+  
- [Node.js](https://nodejs.org/) 20+  
- Visual C++ Build Tools + Windows SDK (for MSVC packages)  
- Or WSL + `x86_64-pc-windows-gnu` cross-compile (see `docs/BUILD.md`)

### Develop

```bash
npm install
npm run tauri:dev
```

### Release artifacts

```bash
npm run build
cd src-tauri && cargo build --release --target x86_64-pc-windows-gnu

./scripts/package-portable.sh
# Windows:
#   .\scripts\package-portable.ps1 -MakeInstaller
```

More: [`docs/BUILD.md`](docs/BUILD.md) · [`docs/RELEASE.md`](docs/RELEASE.md) · [`docs/DDC.md`](docs/DDC.md)

---

## Config & data

| Path | Content |
|:---|:---|
| `%LOCALAPPDATA%\Lumaris\config.json` | Settings, hotkeys, theme, language |
| `%LOCALAPPDATA%\Lumaris\logs\` | Rotating logs (latest 14 files) |

Uninstall keeps user config by default so preferences survive reinstalls.

---

## Roadmap

- [x] DDC/CI external brightness  
- [x] Laptop WMI backlight  
- [x] Multi-monitor stacked UI  
- [x] Dark / light · zh-CN / EN  
- [x] Tray scroll · hotkey acceleration  
- [x] In-app GitHub update check  
- [ ] More VCP features where supported  
- [ ] Signed installer / update channel  

---

## Contributing

Issues and PRs welcome. Before submitting:

```bash
npm run typecheck
npm run build
cargo check --target x86_64-pc-windows-gnu
```

---

## License

[**PolyForm Noncommercial 1.0.0**](LICENSE) © Elnyxn / Lumaris contributors

- ✅ Personal study, research, hobby, and other **noncommercial** uses  
- ✅ View and modify the source under the license terms  
- ❌ **No commercial use** (including paid products, SaaS, and for-profit internal deployments — see LICENSE)  
- Note: this is **source-available**, not OSI “Open Source”

For commercial licensing, contact the maintainers.

---

<p align="center">
  <img src="docs/assets/logo-128.png" width="48" height="48" alt="Lumaris" /><br/>
  <sub>Lumaris — system-grade brightness control for Windows</sub><br/>
  <a href="https://github.com/Elnyxn/Lumaris">github.com/Elnyxn/Lumaris</a>
  ·
  <a href="README.md">简体中文</a>
</p>
