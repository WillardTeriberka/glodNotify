#!/usr/bin/env bash
# 在 Linux 上交叉编译 Windows 可执行文件（方案一）
#
# 前置条件（只需执行一次）:
#   rustup target add x86_64-pc-windows-gnu
#   Debian/Ubuntu:  sudo apt install gcc-mingw-w64-x86-64   (~300-500 MB)
#   Fedora/RHEL:    sudo dnf install mingw64-gcc
#
# 产物: target/x86_64-pc-windows-gnu/release/gold-notify.exe
# 直接拷贝到 Windows 双击即可运行，无需安装任何运行库。
set -euo pipefail

TARGET=x86_64-pc-windows-gnu

echo "==> 检查/安装编译目标 $TARGET"
rustup target add "$TARGET"

echo "==> 开始交叉编译（release）"
cargo build --release --target "$TARGET"

EXE="target/$TARGET/release/gold-notify.exe"
if [ -f "$EXE" ]; then
    SIZE=$(du -h "$EXE" | cut -f1)
    echo "==> 构建完成: $EXE ($SIZE)"
    echo "    拷贝到 Windows 后与 data/ 、.env 放在同一目录即可运行"
else
    echo "!! 未找到产物，请检查构建日志" >&2
    exit 1
fi
