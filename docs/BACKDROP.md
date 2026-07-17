# 窗口背景材质（Windows 10 / 11）

## 目标

亮度浮窗视觉接近 Windows 11 音量/亮度 Flyout：**Transient Acrylic**，非 Mica 主材质。

## Windows 11

通过 `DwmSetWindowAttribute`：

- `DWMWA_SYSTEMBACKDROP_TYPE` = `DWMSBT_TRANSIENTWINDOW`
- `DWMWA_WINDOW_CORNER_PREFERENCE` = `DWMWCP_ROUND`
- `DWMWA_USE_IMMERSIVE_DARK_MODE` = true

## Windows 10

动态加载 `user32!SetWindowCompositionAttribute`：

- `ACCENT_ENABLE_ACRYLICBLURBEHIND`（4）
- 暖灰褐色半透明 `gradient_color`

## 降级链

1. Win11 Transient Acrylic  
2. Win10 Composition Acrylic  
3. CSS `backdrop-filter` + 半透明暖灰（`.fallback-blur`）  
4. 不透明暖灰褐色（`.fallback-solid`）

**禁止**桌面截图模拟模糊。

## 防闪屏

- 窗口默认 `visible: false`、`transparent: true`
- HTML/CSS 根背景强制透明
- `#app` 在 `frontend_ready` 后才 `opacity: 1`
- 首次显示前完成 backdrop 设置

设置页与浮窗共用同一窗口 / 同一 WebView，仅 DOM 切换。
