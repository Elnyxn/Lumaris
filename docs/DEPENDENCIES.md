# 第三方依赖与用途

## 前端 (npm)

| 包 | 用途 |
|----|------|
| `@tauri-apps/api` | IPC / 事件 |
| `@tauri-apps/plugin-autostart` | （API 可选；自启主逻辑在 Rust） |
| `@tauri-apps/plugin-global-shortcut` | （快捷键主逻辑在 Rust 插件） |
| `@tauri-apps/plugin-shell` | 打开日志目录等 |
| `vite` | 构建 |
| `typescript` | 类型检查 |
| `@tauri-apps/cli` | Tauri CLI |

无 React/Vue/Svelte/Tailwind/CDN。

## Rust (crates)

| Crate | 用途 |
|-------|------|
| `tauri` | 应用壳、托盘、窗口 |
| `tauri-plugin-global-shortcut` | 全局快捷键 |
| `tauri-plugin-autostart` | 开机自启 |
| `tauri-plugin-single-instance` | 单实例 |
| `tauri-plugin-shell` | Shell 能力 |
| `windows` / `windows-sys` | Win32 / DDC / DWM / 注册表 |
| `serde` / `serde_json` | 配置与 IPC 序列化 |
| `thiserror` | 错误类型 |
| `tracing` / `tracing-subscriber` / `tracing-appender` | 日志与轮转 |
| `parking_lot` | 低开销锁 |
| `once_cell` | 惰性静态 |
| `dirs` | 用户数据目录 |
| `sha2` / `hex` | 稳定显示器 ID |
| `chrono` | 日志/备份时间戳 |

## 许可证

常见开源许可：MIT / Apache-2.0（Rust 生态与 Tauri 组件）。  
发布前可用 `cargo license` / `npx license-checker` 生成完整清单并附入安装包 `THIRD_PARTY_NOTICES`。
