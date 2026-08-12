#!/bin/zsh

set -eu

LABEL="io.github.rinranx.codex-kimi-bridge"
TARGET_PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
GUI_DOMAIN="gui/$(id -u)"
SERVICE_TARGET="$GUI_DOMAIN/$LABEL"

if /bin/launchctl print "$SERVICE_TARGET" >/dev/null 2>&1; then
  /bin/launchctl bootout "$SERVICE_TARGET"
fi
/bin/launchctl disable "$SERVICE_TARGET" >/dev/null 2>&1 || true

if [[ -e "$TARGET_PLIST" ]]; then
  TRASH_TARGET="$HOME/.Trash/$LABEL.plist.$(date +%Y%m%d-%H%M%S)"
  mv "$TARGET_PLIST" "$TRASH_TARGET"
  print "LaunchAgent 配置已移到废纸篓：$TRASH_TARGET"
else
  print "LaunchAgent 配置不存在，无需删除。"
fi

print "Rust 二进制、Codex 配置、Keychain 和日志均未删除。"
if [[ -t 0 ]]; then
  print "按回车关闭窗口。"
  read -r _
fi
