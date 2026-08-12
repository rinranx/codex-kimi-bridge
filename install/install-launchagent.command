#!/bin/zsh

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LABEL="io.github.rinranx.codex-kimi-bridge"
BRIDGE_BIN="$HOME/.local/bin/codex-kimi-bridge"
TEMPLATE_PLIST="$SCRIPT_DIR/launchagent/$LABEL.plist"
TARGET_DIR="$HOME/Library/LaunchAgents"
TARGET_PLIST="$TARGET_DIR/$LABEL.plist"
LOG_DIR="$HOME/Library/Logs"
STDOUT_LOG="$LOG_DIR/codex-kimi-bridge.log"
STDERR_LOG="$LOG_DIR/codex-kimi-bridge.error.log"
GUI_DOMAIN="gui/$(id -u)"
SERVICE_TARGET="$GUI_DOMAIN/$LABEL"
TEMP_PLIST=""

pause_if_interactive() {
  if [[ -t 0 ]]; then
    print ""
    print "按回车关闭窗口。"
    read -r _
  fi
}

fail() {
  print -u2 "$1"
  pause_if_interactive
  exit 1
}

cleanup() {
  if [[ -n "$TEMP_PLIST" && -e "$TEMP_PLIST" ]]; then
    rm -f "$TEMP_PLIST"
  fi
}

trap cleanup EXIT INT TERM

[[ -x "$BRIDGE_BIN" ]] || fail "尚未安装 Rust 版：$BRIDGE_BIN"
[[ -f "$TEMPLATE_PLIST" ]] || fail "安装包不完整：找不到 $TEMPLATE_PLIST"
/usr/bin/plutil -lint "$TEMPLATE_PLIST" >/dev/null || fail "LaunchAgent 模板不是有效 plist。"

BRIDGE_VERSION="$($BRIDGE_BIN --version 2>/dev/null || true)"
[[ -n "$BRIDGE_VERSION" ]] || fail "无法验证 Rust 桥接版本。"

SERVICE_LOADED=false
if /bin/launchctl print "$SERVICE_TARGET" >/dev/null 2>&1; then
  SERVICE_LOADED=true
fi

LISTENER_PID="$(/usr/sbin/lsof -nP -iTCP:8787 -sTCP:LISTEN -t 2>/dev/null | head -1 || true)"
if [[ -n "$LISTENER_PID" ]]; then
  LISTENER_EXEC="$(/usr/sbin/lsof -a -p "$LISTENER_PID" -d txt -Fn 2>/dev/null | /usr/bin/awk '/^n/{print substr($0,2); exit}' || true)"
  if [[ "$LISTENER_EXEC" != "$BRIDGE_BIN" ]]; then
    fail "8787 正由其他程序占用：${LISTENER_EXEC:-unknown}（PID $LISTENER_PID）。未修改 LaunchAgent。"
  fi
fi

mkdir -p "$TARGET_DIR" "$LOG_DIR"
TEMP_PLIST="$(/usr/bin/mktemp "$TARGET_DIR/.$LABEL.XXXXXX")"
cp "$TEMPLATE_PLIST" "$TEMP_PLIST"
/usr/bin/plutil -remove ProgramArguments.0 "$TEMP_PLIST"
/usr/bin/plutil -insert ProgramArguments.0 -string "$BRIDGE_BIN" "$TEMP_PLIST"
/usr/bin/plutil -replace StandardOutPath -string "$STDOUT_LOG" "$TEMP_PLIST"
/usr/bin/plutil -replace StandardErrorPath -string "$STDERR_LOG" "$TEMP_PLIST"
/usr/bin/plutil -lint "$TEMP_PLIST" >/dev/null || fail "生成的 LaunchAgent 配置无效。"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :ProgramArguments:0' "$TEMP_PLIST")" == "$BRIDGE_BIN" ]] || fail "生成的 LaunchAgent 缺少 Rust 桥接路径。"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :ProgramArguments:1' "$TEMP_PLIST")" == "serve" ]] || fail "生成的 LaunchAgent 缺少 serve 参数。"
if /usr/libexec/PlistBuddy -c 'Print :ProgramArguments:2' "$TEMP_PLIST" >/dev/null 2>&1; then
  fail "生成的 LaunchAgent 含有多余参数。"
fi
chmod 0644 "$TEMP_PLIST"

if [[ "$SERVICE_LOADED" == true ]]; then
  /bin/launchctl bootout "$SERVICE_TARGET" >/dev/null 2>&1 || fail "无法停止现有 LaunchAgent。"
elif [[ -n "$LISTENER_PID" ]]; then
  print "正在把已确认的 Rust 桥接交给 LaunchAgent 管理……"
  kill -TERM "$LISTENER_PID"
  for WAIT_INDEX in {1..40}; do
    if ! kill -0 "$LISTENER_PID" 2>/dev/null; then
      break
    fi
    sleep 0.1
  done
  kill -0 "$LISTENER_PID" 2>/dev/null && fail "现有 Rust 桥接未能正常停止；未加载 LaunchAgent。"
fi

BACKUP_PLIST=""
if [[ -e "$TARGET_PLIST" ]]; then
  BACKUP_PLIST="$TARGET_PLIST.backup.$(date +%Y%m%d-%H%M%S)"
  cp -p "$TARGET_PLIST" "$BACKUP_PLIST"
fi

mv -f "$TEMP_PLIST" "$TARGET_PLIST"
TEMP_PLIST=""
chmod 0644 "$TARGET_PLIST"

if ! /bin/launchctl bootstrap "$GUI_DOMAIN" "$TARGET_PLIST"; then
  if [[ -n "$BACKUP_PLIST" && -e "$BACKUP_PLIST" ]]; then
    mv -f "$BACKUP_PLIST" "$TARGET_PLIST"
    if [[ "$SERVICE_LOADED" == true ]]; then
      /bin/launchctl bootstrap "$GUI_DOMAIN" "$TARGET_PLIST" >/dev/null 2>&1 || true
    fi
  fi
  fail "LaunchAgent 加载失败。"
fi

/bin/launchctl enable "$SERVICE_TARGET"
/bin/launchctl kickstart -k "$SERVICE_TARGET"

HEALTH_JSON=""
for WAIT_INDEX in {1..40}; do
  HEALTH_JSON="$(/usr/bin/curl -fsS --max-time 1 http://127.0.0.1:8787/health 2>/dev/null || true)"
  if print -r -- "$HEALTH_JSON" | /usr/bin/grep -Eq '"implementation"[[:space:]]*:[[:space:]]*"rust"'; then
    break
  fi
  sleep 0.25
done

if ! print -r -- "$HEALTH_JSON" | /usr/bin/grep -Eq '"implementation"[[:space:]]*:[[:space:]]*"rust"'; then
  fail "LaunchAgent 已加载，但 Rust 健康检查未通过。请查看 $STDERR_LOG"
fi

print ""
print "LaunchAgent 安装完成。"
print "标签：$LABEL"
print "桥接版本：$BRIDGE_VERSION (Rust)"
print "配置：$TARGET_PLIST"
print "日志：$STDOUT_LOG"
print "错误日志：$STDERR_LOG"
print "登录后会自动启动；异常退出时由 launchd 恢复。"
print "未调用 Kimi 上游。"
pause_if_interactive
