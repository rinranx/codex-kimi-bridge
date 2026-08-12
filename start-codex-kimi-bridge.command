#!/bin/zsh

set -eu

BRIDGE_BIN="$HOME/.local/bin/codex-kimi-bridge"

if [[ ! -x "$BRIDGE_BIN" ]]; then
  print -u2 "没有在 ~/.local/bin 找到 Rust 版 codex-kimi-bridge。"
  print -u2 "请下载完整安装包并运行 install-codex-kimi-bridge.command。"
  exit 1
fi

exec "$BRIDGE_BIN" serve "$@"
