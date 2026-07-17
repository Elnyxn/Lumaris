# 发布与安装包

## 一、产物形态

| 形态 | 文件 | 适用 |
|------|------|------|
| 便携版 | `Lumaris-portable-x.y.z.zip` | 解压即用，不写开始菜单 |
| 安装程序 | `Lumaris-Setup-x.y.z.exe`（Inno） | 向导安装、快捷方式、卸载 |
| Tauri NSIS/MSI | `src-tauri/target/.../bundle/` | 在 **Windows 本机** 用 `tauri build` |

### 自动发布（推荐）

仓库已配置 GitHub Actions：`.github/workflows/release.yml`。

1. 对齐版本号（三处必须一致）：
   - `package.json` → `version`
   - `src-tauri/tauri.conf.json` → `version`
   - `src-tauri/Cargo.toml` → `version`
2. 提交并推送后打 tag：

```bash
git tag v1.0.1
git push origin v1.0.1
```

3. Actions 在 `windows-latest` 上自动：
   - `npm ci` + 前端 build + `cargo build --release`
   - 打包 Portable zip
   - 安装 Inno Setup 并生成 Setup.exe
   - 写入 `SHA256SUMS.txt`
   - 创建/更新 GitHub Release，并挂上上述资产

应用内「检查更新」读取的即该 Release 的 latest tag。

---

## 二、你现在的 WSL 交叉编译流程（推荐 + Inno）

### 1. 编译

```bash
cd /path/to/Lumaris
npm run build
cd src-tauri
cargo build --release --target x86_64-pc-windows-gnu
```

产物：`src-tauri/target/x86_64-pc-windows-gnu/release/lumaris.exe`

### 2. 组装便携目录 / zip

```bash
chmod +x scripts/package-portable.sh
./scripts/package-portable.sh
# 或指定 exe：
./scripts/package-portable.sh /mnt/f/Lumaris/Lumaris.exe
```

得到：

- `release/portable/` — 可直接拷贝分发
- `release/Lumaris-portable-1.0.0.zip`
- `installer/payload/` — 给 Inno 用

### 3. 做安装程序（Windows 上）

1. 安装 [Inno Setup 6](https://jrsoftware.org/isinfo.php)（含中文语言包）
2. 在 **Windows** 打开项目，确认 `installer/payload/Lumaris.exe` 已存在
3. 任选其一：

```powershell
# 自动 ISCC
.\scripts\package-portable.ps1 -MakeInstaller

# 或 Inno 图形界面打开 installer\Lumaris.iss → Compile
# 或：
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" installer\Lumaris.iss
```

输出：`installer/output/Lumaris-Setup-1.0.0.exe`

用户双击 Setup 即可安装（默认当前用户、可选桌面图标/开机启动）。

---

## 三、Tauri 官方安装包（需 Windows 开发机）

`tauri.conf.json` 已配置 `nsis` + `msi`。

在 **Windows + MSVC + NSIS** 环境：

```powershell
npm install
npm run tauri:build
```

产物大致在：

```
src-tauri\target\release\bundle\nsis\*.exe
src-tauri\target\release\bundle\msi\*.msi
```

说明：

- NSIS：`installMode: currentUser`（一般不要管理员）
- WebView2：`downloadBootstrapper`（缺运行时会引导下载）
- 卸载默认**不删** `%LOCALAPPDATA%\Lumaris` 配置

WSL 交叉编译 **不能** 完整替代 `tauri build` 的 Windows 安装包链路；WSL 适合出 exe，再用 Inno 包安装程序。

---

## 四、版本号对齐

发布前改三处为同一版本：

1. `package.json` → `version`
2. `src-tauri/Cargo.toml` → `version`
3. `src-tauri/tauri.conf.json` → `version`
4. （Inno）`installer/Lumaris.iss` → `#define MyAppVersion`

---

## 五、检查清单

- [ ] 双屏 / 仅第二屏幕 / 笔记本 WMI 回归
- [ ] 托盘、快捷键、设置保存
- [ ] 中英文切换（设置 → 界面 → 语言）
- [ ] 深色 / 浅色
- [ ] 安装包安装 / 卸载 / 开机自启任务（若勾选）
- [ ] 干净机或虚拟机测 WebView2 引导

---

## 六、语言包

前端：`src/i18n/locales/zh-CN.ts`、`en.ts`  
后端托盘：`src-tauri/src/i18n/mod.rs`  

设置 → 界面 → 语言；配置字段 `locale`（`zh-CN` | `en`）。
