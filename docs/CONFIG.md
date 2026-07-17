# 配置结构

路径：`%LOCALAPPDATA%\Lumaris\config.json`

## 字段（schemaVersion = 1）

| 字段 | 类型 | 说明 |
|------|------|------|
| schemaVersion | number | 配置版本 |
| hotkeys | object | 各动作快捷键字符串或 null |
| stepPercent | 0/1/2/5/10 | 0 表示使用 custom |
| customStepPercent | 1–25 | 自定义步长 |
| autostart | bool | 偏好（以系统 Run 键为准） |
| silentStartup | bool | 启动静默 |
| delayedMonitorInit | bool | 登录后延迟枚举 |
| delayedInitMs | number | 延迟毫秒 |
| osd.enabled | bool | 是否自动显示 OSD |
| osd.autoHideMs | number | 自动隐藏 |
| ui.animations | bool | 动画 |
| ui.opacity | 0.5–1 | 面板透明度 |
| targetMode | enum | mouseMonitor / lastUsed / primary / fixed / allSync |
| lastMonitorId | string? | 最近设备 |
| fixedMonitorId | string? | 固定目标 |
| layoutMode | single \| list | 浮窗布局 |
| syncAll | bool | 同步模式 |
| rememberLastMonitor | bool | 记忆最近 |
| readBrightnessOnStart | bool | 启动读真实亮度 |
| monitorAliases | map | 别名 |
| monitorSyncInclude | map | 是否参与同步 |
| cachedBrightness | map | 缓存亮度 |
| logLevel | info/debug/warn/error | 日志级别 |

## 写入策略

1. 序列化到 `config.json.tmp`
2. `sync` 后 rename 替换
3. 损坏时备份为 `config.json.corrupt.<时间戳>` 并恢复默认
4. 滑块拖动中不写盘；结束后延迟约 800ms 合并保存
5. 退出前强制保存

## 迁移

`settings/migration.rs`：按 `schemaVersion` 链式升级。高于当前版本则报错并回退默认（不崩溃）。
