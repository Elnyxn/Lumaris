# 已执行检查记录

日期：2026-07-16  
环境：WSL2 Linux (`x86_64-unknown-linux-gnu`)，交叉目标 `x86_64-pc-windows-gnu`

## 已通过

| 检查 | 结果 |
|------|------|
| `npm install` | 通过 |
| `npm run typecheck` (`tsc --noEmit`) | 通过 |
| `npm run build` (Vite production) | 通过 → `dist/` |
| `cargo check --target x86_64-pc-windows-gnu` | 通过 |
| `cargo test --target x86_64-pc-windows-gnu --lib --no-run` | 通过（已编译 Windows 测试二进制） |

## 未在本环境执行（需 Windows 主机 + 真机显示器）

| 检查 | 原因 |
|------|------|
| `cargo test` 运行时 | 测试二进制为 Windows PE，无法在 Linux 直接执行 |
| `npm run tauri:build` / NSIS/MSI | 需 MSVC 工具链与 WebView2 打包环境 |
| 真实 DDC/CI 读写 | 需外接显示器硬件 |
| 空闲内存/CPU 性能测量 | 需 Release 在 Windows 实测 |
| 热插拔 / 睡眠唤醒 / 托盘 Explorer 恢复 | 需 Windows 系统事件 |

## 说明

- 未虚构任何硬件测试数据。
- Linux 本地 `cargo test` 会因 Tauri 依赖 GTK/`gobject` 失败，属预期；产品目标平台为 Windows。
- 请在 Windows 上执行：

```powershell
cd lumaris
npm install
npm run tauri:dev
# 或
npm run tauri:build
cd src-tauri
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
