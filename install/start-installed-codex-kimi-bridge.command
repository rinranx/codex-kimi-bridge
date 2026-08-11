#!/bin/zsh

set -eu

if command -v codex-kimi-bridge >/dev/null 2>&1; then
  BRIDGE_BIN="$(command -v codex-kimi-bridge)"
elif [[ -x /opt/homebrew/bin/codex-kimi-bridge ]]; then
  BRIDGE_BIN="/opt/homebrew/bin/codex-kimi-bridge"
elif [[ -x /usr/local/bin/codex-kimi-bridge ]]; then
  BRIDGE_BIN="/usr/local/bin/codex-kimi-bridge"
else
  print -u2 "codex-kimi-bridge is not installed. Follow INSTALL-GUIDE.zh-CN.md first."
  exit 1
fi

exec "$BRIDGE_BIN" serve
