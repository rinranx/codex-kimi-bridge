#!/bin/zsh

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_BIN="$SCRIPT_DIR/bin/codex-kimi-bridge"
CHECKSUM_FILE="$SCRIPT_DIR/BINARY-SHA256.txt"
TARGET_DIR="$HOME/.local/bin"
TARGET_BIN="$TARGET_DIR/codex-kimi-bridge"
EXPECTED_VERSION="0.4.0"

if [[ ! -f "$SOURCE_BIN" ]]; then
  print -u2 "安装包不完整：找不到 bin/codex-kimi-bridge。"
  print -u2 "Please download and extract the complete installation kit again."
  exit 1
fi

if [[ ! -f "$CHECKSUM_FILE" ]]; then
  print -u2 "安装包不完整：找不到 BINARY-SHA256.txt。"
  exit 1
fi

if ! (cd "$SCRIPT_DIR" && shasum -a 256 -c BINARY-SHA256.txt); then
  print -u2 "二进制 SHA-256 校验失败，安装器没有修改任何文件。"
  exit 1
fi

SOURCE_VERSION="$($SOURCE_BIN --version 2>/dev/null || true)"
if [[ "$SOURCE_VERSION" != "$EXPECTED_VERSION" ]]; then
  print -u2 "安装包校验失败：预期 $EXPECTED_VERSION，实际 ${SOURCE_VERSION:-unknown}。"
  exit 1
fi

RESOLVED_BIN="$(command -v codex-kimi-bridge 2>/dev/null || true)"
if [[ -n "$RESOLVED_BIN" && "$RESOLVED_BIN" != "$TARGET_BIN" ]]; then
  print -u2 "发现另一个同名命令：$RESOLVED_BIN"
  print -u2 "为避免命令冲突，安装器没有修改任何文件。"
  print -u2 "如果这是旧的 npm 0.1.0，请先运行："
  print -u2 "  npm uninstall --global codex-kimi-bridge"
  print -u2 "确认 command -v codex-kimi-bridge 不再指向旧位置后，再运行本安装器。"
  exit 1
fi

mkdir -p "$TARGET_DIR"

if [[ -e "$TARGET_BIN" ]]; then
  CURRENT_VERSION="$($TARGET_BIN --version 2>/dev/null || true)"
  print "将把 ${CURRENT_VERSION:-unknown} 更新为 $EXPECTED_VERSION。"
  print -n "继续并保留一份备份？[y/N] "
  read -r REPLY
  if [[ "$REPLY" != "y" && "$REPLY" != "Y" ]]; then
    print "已取消，未修改现有文件。"
    exit 0
  fi
  BACKUP_BIN="$TARGET_BIN.backup.$(date +%Y%m%d-%H%M%S)"
  cp -p "$TARGET_BIN" "$BACKUP_BIN"
  print "旧版本已备份到：$BACKUP_BIN"
fi

TEMP_BIN="$TARGET_DIR/.codex-kimi-bridge.install.$$"
trap 'rm -f "$TEMP_BIN"' EXIT INT TERM
cp "$SOURCE_BIN" "$TEMP_BIN"
chmod 755 "$TEMP_BIN"
mv -f "$TEMP_BIN" "$TARGET_BIN"
trap - EXIT INT TERM

INSTALLED_VERSION="$($TARGET_BIN --version)"
if [[ "$INSTALLED_VERSION" != "$EXPECTED_VERSION" ]]; then
  print -u2 "安装后的版本校验失败。"
  exit 1
fi

print ""
print "安装完成：$TARGET_BIN"
print "版本：$INSTALLED_VERSION (Rust)"
print ""
print "安装器没有修改 Codex 配置、Keychain 或 shell PATH。"
print "下一步请阅读 install/INSTALL-GUIDE.zh-CN.md，配置完成后从三种启动方式中任选一种。"
print "审核后安装签名任务交接 Hooks：$TARGET_BIN hooks install"
print "随后完全重启 Codex Desktop，输入 /hooks，信任 UserPromptSubmit 与 PreToolUse（Agent / spawn_agent / collaborationspawn_agent / collaboration.spawn_agent）。"
if [[ -t 0 ]]; then
  print ""
  print "按回车关闭窗口。"
  read -r _
fi
