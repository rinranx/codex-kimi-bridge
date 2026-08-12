#!/bin/zsh

set -eu

if command -v codex-kimi-bridge-node >/dev/null 2>&1; then
  BRIDGE_BIN="$(command -v codex-kimi-bridge-node)"
elif [[ -x /opt/homebrew/bin/codex-kimi-bridge-node ]]; then
  BRIDGE_BIN="/opt/homebrew/bin/codex-kimi-bridge-node"
elif [[ -x /usr/local/bin/codex-kimi-bridge-node ]]; then
  BRIDGE_BIN="/usr/local/bin/codex-kimi-bridge-node"
else
  print -u2 "codex-kimi-bridge-node is not installed. Follow INSTALL-GUIDE.zh-CN.md first."
  exit 1
fi

exec "$BRIDGE_BIN" serve
