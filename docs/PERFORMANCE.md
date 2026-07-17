# 性能测量方法

在 **Release** 构建、启动并空闲 **60 秒** 后测量。

## 建议工具

- 任务管理器 / 资源监视器  
- Process Explorer  
- PowerShell `Get-Process`

## 指标

1. `Lumaris.exe` Private Working Set  
2. 所有关联 `msedgewebview2.exe` Private Working Set  
3. 两者之和（**不得只报主进程**）  
4. 主进程线程数  
5. 主进程句柄数  
6. 空闲 60s 平均 CPU  
7. 连续调亮度 30s 平均 CPU  
8. 隐藏后是否仍有 `requestAnimationFrame` / 高频 `setInterval` / 高频 IPC  
9. 开关浮窗 100 次后内存/句柄变化  
10. 热插拔、睡眠恢复后资源变化  

## 目标

- 空闲 CPU 接近 0%  
- 单 WebView2  
- 无固定频率 DDC 轮询  
- 总内存尽量 &lt; 100MB（视 WebView2 版本与系统而定）  
- 无持续句柄/Private Bytes 增长  

## 当前环境说明

本交付在 **WSL2 Linux** 环境完成源码与前端构建校验；**无法在此环境链接 Windows DDC / 测量真实工作集**。  
请在 Windows 主机执行 `npm run tauri:build` 后按上表实测，并如实记录机器型号与显示器型号。

示例记录模板：

```
环境: Windows 11 24H2 / 16GB / 外接 DELL U2720Q
空闲 60s: 主进程 __ MB + WebView2 __ MB = __ MB
空闲 CPU: __ %
句柄: __
开关浮窗 100 次 ΔPrivate: __ MB
```
