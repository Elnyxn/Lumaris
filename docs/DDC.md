# DDC/CI 实现说明

## 控制路径

1. `EnumDisplayMonitors` → 逻辑 `HMONITOR`
2. `GetMonitorInfoW` / `EnumDisplayDevicesW` → 设备名、主屏、工作区
3. `GetNumberOfPhysicalMonitorsFromHMONITOR` + `GetPhysicalMonitorsFromHMONITOR`
4. 每台物理显示器独立设备，**不合并同型号**
5. 亮度控制优先级：
   - `GetMonitorCapabilities` + `GetMonitorBrightness` / `SetMonitorBrightness`
   - 失败则 `GetVCPFeatureAndVCPFeatureReply` / `SetVCPFeature`，VCP = **0x10**
6. 均失败 → `Unsupported`，不崩溃

## 范围换算

- 读取保留 min / max / current 原始值
- UI 统一 **0–100%**
- 写入：`raw = min + round(percent * (max - min) / 100)`
- **不假设**所有显示器范围为 0–100

## 句柄生命周期

- `PHYSICAL_MONITOR` 由缓存持有；重新枚举前 `DestroyPhysicalMonitors`
- 禁止复制所有权；退出时清空缓存
- DDC 操作仅在固定工作线程执行，避免 UI 阻塞与 UAF

## 工作线程

- 单线程 `lumaris-ddc`
- 命令：`SetBrightness` / `ReadBrightness` / `RefreshMonitors` / `Pause` / `Resume` / `Shutdown`
- 连续写入合并为最新值；`final_write=true` 保证最终落盘到硬件
- 失败有限重试（默认 2）+ 约 3s 冷却，避免拖死整条队列
- 空闲 `recv` 阻塞，无忙等

## 状态

| 状态 | 含义 |
|------|------|
| Available | 实时可读可控 |
| Cached | 使用缓存亮度 |
| TemporarilyOffline | 冷却/暂无响应 |
| Unsupported | 无 DDC |
| ReadFailed / WriteFailed | 最近操作失败 |
| Sleeping | 系统睡眠挂起 |
| Disconnected | 设备已移除 |

## 稳定 ID

组合 `DeviceName` / `DeviceID` / `DeviceString` / 物理描述 / 索引等，SHA-256 截断为 `mon_<hex>`。非随机 UUID。

## 限制

需真实外接显示器且开启 DDC/CI。虚拟机、部分集显坞站、关闭了显示器 OSD 中 DDC 时会 Unsupported。
