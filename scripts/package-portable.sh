#!/usr/bin/env bash
# WSL / Linux：把已交叉编译的 Windows release 组装成便携目录 + zip
# 用法：
#   ./scripts/package-portable.sh
#   ./scripts/package-portable.sh /path/to/Lumaris.exe

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/release/portable"
PAYLOAD="$ROOT/installer/payload"
TAURI_CONFIG="$ROOT/src-tauri/tauri.conf.json"
PACKAGE_JSON="$ROOT/package.json"

command -v node >/dev/null 2>&1 || {
  echo "缺少 node，无法读取项目版本" >&2
  exit 1
}

VER="$(node -e '
const fs = require("fs");
const tauri = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const pkg = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
if (!tauri.version || tauri.version !== pkg.version) {
  console.error(`版本不一致: tauri=${tauri.version ?? "<missing>"}, package=${pkg.version ?? "<missing>"}`);
  process.exit(1);
}
process.stdout.write(tauri.version);
' "$TAURI_CONFIG" "$PACKAGE_JSON")"

EXE_CANDIDATES=(
  "$ROOT/src-tauri/target/x86_64-pc-windows-gnu/release/lumaris.exe"
  "$ROOT/src-tauri/target/x86_64-pc-windows-msvc/release/Lumaris.exe"
  "$ROOT/src-tauri/target/release/Lumaris.exe"
)

EXE=""
if [[ -n "${1:-}" ]]; then
  [[ -f "$1" ]] || {
    echo "指定的 Lumaris.exe 不存在: $1" >&2
    exit 1
  }
  EXE="$1"
else
  for c in "${EXE_CANDIDATES[@]}"; do
    if [[ -f "$c" ]]; then
      EXE="$c"
      break
    fi
  done
fi

if [[ -z "$EXE" ]]; then
  echo "找不到 lumaris.exe，请先 cargo build --release --target x86_64-pc-windows-gnu" >&2
  exit 1
fi

EXE_DIR="$(dirname "$EXE")"
echo "使用: $EXE"
rm -rf -- "$OUT" "$PAYLOAD"
mkdir -p "$OUT" "$PAYLOAD" "$ROOT/release"
cp -f "$EXE" "$OUT/Lumaris.exe"
cp -f "$EXE" "$PAYLOAD/Lumaris.exe"

# 运行库：优先 exe 同目录，其次 mingw 工具链路径
copy_dll() {
  local name="$1"
  local src=""
  if [[ -f "$EXE_DIR/$name" ]]; then
    src="$EXE_DIR/$name"
  elif command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    local cand
    cand="$(x86_64-w64-mingw32-gcc -print-file-name="$name" 2>/dev/null || true)"
    if [[ -n "$cand" && -f "$cand" ]]; then
      src="$cand"
    fi
  fi
  if [[ -n "$src" ]]; then
    cp -f "$src" "$OUT/$name"
    cp -f "$src" "$PAYLOAD/$name"
    echo "  + $name"
  else
    echo "  ! 缺少 $name（运行时可能需要）" >&2
  fi
}

echo "打包运行库…"
for name in WebView2Loader.dll libgcc_s_seh-1.dll libwinpthread-1.dll libstdc++-6.dll; do
  copy_dll "$name"
done

cat > "$OUT/README.txt" <<EOF
Lumaris portable $VER
====================
双击 Lumaris.exe 运行。

配置目录：
  %LOCALAPPDATA%\\Lumaris

依赖：
  Microsoft Edge WebView2 Runtime（Win10/11 通常已自带）

官网/反馈：见项目仓库
EOF

# 同步中文说明到 payload（安装包同目录无 README 也可）
cp -f "$OUT/README.txt" "$PAYLOAD/README.txt" 2>/dev/null || true

ZIP="$ROOT/release/Lumaris-portable-${VER}.zip"
rm -f "$ZIP"
( cd "$OUT" && zip -qr "$ZIP" . )
echo "便携包: $ZIP"
echo "Inno payload: $PAYLOAD"

# 可选：若本机有 ISCC，直接出 Setup
ISCC=""
for c in \
  "/mnt/c/Program Files (x86)/Inno Setup 6/ISCC.exe" \
  "/mnt/c/Program Files/Inno Setup 6/ISCC.exe" \
  "$HOME/.local/share/Inno Setup 6/ISCC.exe"
do
  if [[ -f "$c" ]]; then ISCC="$c"; break; fi
done

if [[ -n "$ISCC" ]]; then
  echo "使用 ISCC: $ISCC"
  # 路径转 Windows
  WIN_ISS="$(wslpath -w "$ROOT/installer/Lumaris.iss" 2>/dev/null || echo "$ROOT/installer/Lumaris.iss")"
  "$ISCC" //O"$(wslpath -w "$ROOT/installer/output" 2>/dev/null || echo "$ROOT/installer/output")" "$WIN_ISS" \
    || cmd.exe /c "\"$ISCC\" \"$(wslpath -w "$ROOT/installer/Lumaris.iss")\""
  echo "安装包目录: $ROOT/installer/output/"
  ls -la "$ROOT/installer/output/" 2>/dev/null || true
else
  echo "未检测到 Inno Setup 6。安装后执行："
  echo "  powershell -File scripts\\package-portable.ps1 -MakeInstaller"
  echo "  或 ISCC installer\\Lumaris.iss"
fi

echo ""
echo "=== 发布产物 ==="
ls -la "$ZIP" 2>/dev/null || true
ls -la "$ROOT/installer/output/"*.exe 2>/dev/null || true
