# 快捷键与开机自启

## 快捷键

- 插件：`tauri-plugin-global-shortcut`（RegisterHotKey 路径，非 WH_KEYBOARD_LL）
- 修改流程：校验 → 注销旧键 → 尝试注册新键 → 成功才写配置 → 失败恢复旧键
- 应用内冲突：保存前比对其他动作
- 系统占用：注册失败返回「该快捷键可能已被系统或其他程序占用。」
- 单个键失败不阻止应用启动

## 开机自启

1. 使用 `tauri-plugin-autostart`，参数 `--startup`
2. Windows 上额外确保 `HKCU\...\Run\Lumaris` 值为 `"exe路径" --startup`
3. 设置页「开机自动启动」读取**系统实际状态**，失败不显示为已启用
4. 禁用时删除 Run 值并 `autolaunch().disable()`
5. 无 UAC、无计划任务默认方案、不写 `HKLM`

### `--startup` 行为

- 不显示浮窗、不抢焦点、无欢迎页、无控制台
- 错误进日志；托盘就绪后常驻
