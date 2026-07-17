# 构建与发布

## 开发环境

1. 安装 Rust（`rustup`），默认 host `x86_64-pc-windows-msvc`
2. 安装 Node.js 20+
3. 安装 VS Build Tools：Workload **Desktop development with C++**，含 Windows 10/11 SDK
4. 确认 WebView2 Runtime：`Get-Command msedgewebview2` 或系统组件列表

```powershell
rustc -V
cargo -V
node -v
npm -v
```

## 命令

```powershell
cd lumaris
npm install

# Debug 开发
npm run tauri:dev

# 仅前端
npm run dev
npm run typecheck
npm run build

# Release
npm run tauri:build
```

Rust 侧：

```powershell
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

## 子系统与 DPI

- Release：`windows_subsystem = "windows"`，无控制台
- Debug：可保留控制台便于日志
- 应用清单由 Tauri 生成；DPI 感知走 Per-Monitor V2（WebView2 / 现代 Windows 默认路径）
- 窗口定位使用目标显示器 `GetDpiForMonitor` 工作区物理像素

## 图标

`src-tauri/icons/` 已包含 16–256 ICO 与各尺寸 PNG。替换时可用：

```powershell
# 任意图片 → 使用 tauri icon
npm run tauri icon path\to\icon.png
```

## 安装包 / 便携版

完整流程见 **[RELEASE.md](./RELEASE.md)**。

简要：

```bash
# WSL：编译 + 便携 zip
npm run build
cd src-tauri && cargo build --release --target x86_64-pc-windows-gnu
./scripts/package-portable.sh
```

```powershell
# Windows：Inno 安装程序
.\scripts\package-portable.ps1 -MakeInstaller
# 或官方 Tauri 包（需本机 NSIS/MSVC）
npm run tauri:build
```

`tauri.conf.json` 已开 `nsis` + `msi`；Inno 脚本：`installer/Lumaris.iss`。

## 版本号

- 前端：`package.json` → `version`
- Rust：`src-tauri/Cargo.toml` → `version`
- 打包：`src-tauri/tauri.conf.json` → `version`
- Inno：`installer/Lumaris.iss` → `MyAppVersion`

三者保持一致（当前 `1.0.0`）。

## 代码签名（预留）

```json
"windows": {
  "certificateThumbprint": "<你的证书指纹>",
  "digestAlgorithm": "sha256",
  "timestampUrl": "http://timestamp.digicert.com"
}
```

未签名时部分安全软件可能提示，属预期；**不使用 UPX**。

## WebView2 缺失

打包器可引导下载 Bootstrapper。若运行时仍失败，用户可安装：

https://developer.microsoft.com/microsoft-edge/webview2/

## 关闭调试

- Release 构建默认关闭高频日志（`log_level: info`）
- 设置页可将日志设为 `error`
- 不设置环境变量 `RUST_LOG` 即可保持安静
