#!/bin/zsh

set -eu

BRIDGE_BIN="$HOME/.local/bin/codex-kimi-bridge"

if [[ ! -x "$BRIDGE_BIN" ]]; then
  print -u2 "尚未安装 Rust 版 codex-kimi-bridge。"
  print -u2 "请先双击 install-codex-kimi-bridge.command。"
  exit 1
fi

print "启动 $($BRIDGE_BIN --version) (Rust)。"
print "保持本窗口打开；需要停止时按 Control+C。"
print ""
exec "$BRIDGE_BIN" serve "$@"
