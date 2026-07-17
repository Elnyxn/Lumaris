# 组装便携版目录，并可选调用 Inno Setup 生成安装包
# 在 Windows PowerShell 中执行：
#   .\scripts\package-portable.ps1
#   .\scripts\package-portable.ps1 -MakeInstaller
#   .\scripts\package-portable.ps1 -SourceExe C:\path\to\Lumaris.exe

param(
  [string]$SourceExe = "",
  [string]$OutDir = "",
  [switch]$MakeInstaller
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $OutDir) { $OutDir = Join-Path $Root "release\portable" }
$Payload = Join-Path $Root "installer\payload"
$TauriConfig = Get-Content -Raw (Join-Path $Root "src-tauri\tauri.conf.json") | ConvertFrom-Json
$PackageConfig = Get-Content -Raw (Join-Path $Root "package.json") | ConvertFrom-Json
$Version = [string]$TauriConfig.version
if (-not $Version -or $Version -ne [string]$PackageConfig.version) {
  throw "版本不一致: tauri=$Version, package=$($PackageConfig.version)"
}

# 候选产物路径
$candidates = @(@(
  (Join-Path $Root "src-tauri\target\x86_64-pc-windows-msvc\release\Lumaris.exe"),
  (Join-Path $Root "src-tauri\target\x86_64-pc-windows-gnu\release\lumaris.exe"),
  (Join-Path $Root "src-tauri\target\release\Lumaris.exe")
) | Where-Object { $_ -and (Test-Path $_ -PathType Leaf) })

if ($SourceExe) {
  if (-not (Test-Path $SourceExe -PathType Leaf)) {
    throw "指定的 Lumaris.exe 不存在: $SourceExe"
  }
  $exe = (Resolve-Path $SourceExe).Path
} elseif (-not $candidates -or $candidates.Count -eq 0) {
  Write-Error "找不到 Lumaris.exe。请先编译 release，或用 -SourceExe 指定路径。"
} else {
  $exe = $candidates[0]
}

$exeDir = Split-Path -Parent $exe
Write-Host "使用: $exe"

foreach ($stagingDir in @($OutDir, $Payload)) {
  $fullPath = [System.IO.Path]::GetFullPath($stagingDir)
  $pathRoot = [System.IO.Path]::GetPathRoot($fullPath)
  if ($fullPath -eq $pathRoot -or $fullPath -eq [System.IO.Path]::GetFullPath($Root)) {
    throw "拒绝清理不安全的 staging 路径: $fullPath"
  }
  if (Test-Path $fullPath) { Remove-Item $fullPath -Recurse -Force }
}
New-Item -ItemType Directory -Force -Path $OutDir, $Payload | Out-Null
Copy-Item -Force $exe (Join-Path $OutDir "Lumaris.exe")
Copy-Item -Force $exe (Join-Path $Payload "Lumaris.exe")

# 同目录 DLL
foreach ($name in @("WebView2Loader.dll", "libgcc_s_seh-1.dll", "libwinpthread-1.dll", "libstdc++-6.dll")) {
  $src = Join-Path $exeDir $name
  if (Test-Path $src) {
    Copy-Item -Force $src (Join-Path $OutDir $name)
    Copy-Item -Force $src (Join-Path $Payload $name)
  }
}

# 说明文件
@"
Lumaris 便携版 $Version
==============
双击 Lumaris.exe 运行。配置保存在:
  %LOCALAPPDATA%\Lumaris

建议系统已安装 Microsoft Edge WebView2 Runtime。
"@ | Set-Content -Encoding UTF8 (Join-Path $OutDir "README.txt")

# zip
$zip = Join-Path $Root "release\Lumaris-portable-$Version.zip"
New-Item -ItemType Directory -Force -Path (Split-Path $zip) | Out-Null
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $OutDir "*") -DestinationPath $zip
Write-Host "便携包: $zip"

if ($MakeInstaller) {
  $iscc = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
  ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

  if (-not $iscc) {
    Write-Warning "未找到 Inno Setup 6 (ISCC.exe)。请安装后重试 -MakeInstaller"
  } else {
    & $iscc "/DMyAppVersion=$Version" (Join-Path $Root "installer\Lumaris.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC 失败: $LASTEXITCODE" }
    $outDir = Join-Path $Root "installer\output"
    $setup = Get-ChildItem $outDir -Filter "Lumaris-Setup-*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($setup) {
      New-Item -ItemType Directory -Force -Path (Join-Path $Root "release") | Out-Null
      Copy-Item -Force $setup.FullName (Join-Path $Root "release\$($setup.Name)")
    }
    Write-Host "安装包输出: installer\output\"
  }
}

Write-Host "完成。"
