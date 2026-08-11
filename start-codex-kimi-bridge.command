#!/bin/zsh

set -eu

BRIDGE_DIR="$(cd "$(dirname "$0")" && pwd)"

if command -v node >/dev/null 2>&1; then
  NODE_BIN="$(command -v node)"
elif [[ -x /opt/homebrew/bin/node ]]; then
  NODE_BIN="/opt/homebrew/bin/node"
elif [[ -x /usr/local/bin/node ]]; then
  NODE_BIN="/usr/local/bin/node"
else
  print -u2 "Node.js 20+ was not found. Install Node.js, then run this launcher again."
  exit 1
fi

exec "$NODE_BIN" "$BRIDGE_DIR/bin/codex-kimi-bridge.mjs" serve
