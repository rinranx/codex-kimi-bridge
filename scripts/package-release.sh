#!/bin/bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version> <empty-output-directory>" >&2
  exit 64
fi

VERSION="$1"
OUTPUT_INPUT="$2"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd -P)"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid release version: $VERSION" >&2
  exit 64
fi

mkdir -p "$OUTPUT_INPUT"
OUTPUT_DIR="$(cd "$OUTPUT_INPUT" && pwd -P)"
if [[ "$OUTPUT_DIR" == "/" || "$OUTPUT_DIR" == "$PROJECT_DIR" ]]; then
  echo "refusing unsafe output directory: $OUTPUT_DIR" >&2
  exit 64
fi
if [[ -n "$(find "$OUTPUT_DIR" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "output directory must be empty: $OUTPUT_DIR" >&2
  exit 64
fi

CARGO_VERSION="$(awk -F '"' '/^version = / { print $2; exit }' "$PROJECT_DIR/rust/Cargo.toml")"
if [[ "$CARGO_VERSION" != "$VERSION" ]]; then
  echo "Cargo version $CARGO_VERSION does not match release version $VERSION" >&2
  exit 1
fi

ARM_SOURCE="$PROJECT_DIR/rust/target/aarch64-apple-darwin/release/codex-kimi-bridge"
INTEL_SOURCE="$PROJECT_DIR/rust/target/x86_64-apple-darwin/release/codex-kimi-bridge"
for SOURCE_BINARY in "$ARM_SOURCE" "$INTEL_SOURCE"; do
  if [[ ! -x "$SOURCE_BINARY" ]]; then
    echo "missing release binary: $SOURCE_BINARY" >&2
    exit 1
  fi
  if [[ "$($SOURCE_BINARY --version)" != "$VERSION" ]]; then
    echo "binary has the wrong version: $SOURCE_BINARY" >&2
    exit 1
  fi
done

STAGE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codex-kimi-bridge-release.XXXXXX")"
cleanup() {
  if [[ -n "${STAGE_DIR:-}" && -d "$STAGE_DIR" ]]; then
    case "$(basename "$STAGE_DIR")" in
      codex-kimi-bridge-release.*) /bin/rm -rf "$STAGE_DIR" ;;
    esac
  fi
}
trap cleanup EXIT INT TERM

copy_release_docs() {
  local DESTINATION="$1"
  cp -p "$PROJECT_DIR/README.md" "$DESTINATION/README.md"
  cp -p "$PROJECT_DIR/README.en.md" "$DESTINATION/README.en.md"
  cp -p "$PROJECT_DIR/LICENSE" "$DESTINATION/LICENSE"
  cp -p "$PROJECT_DIR/SECURITY.md" "$DESTINATION/SECURITY.md"
  cp -p "$PROJECT_DIR/PROVENANCE.md" "$DESTINATION/PROVENANCE.md"
}

KIT_NAME="codex-kimi-bridge-macos-install-kit-$VERSION"
KIT_DIR="$STAGE_DIR/$KIT_NAME"
mkdir -p \
  "$KIT_DIR/bin" \
  "$KIT_DIR/install" \
  "$KIT_DIR/launchagent" \
  "$KIT_DIR/templates" \
  "$KIT_DIR/companion-skill/manage-codex-kimi-bridge/agents"

copy_release_docs "$KIT_DIR"
cp -p "$PROJECT_DIR/INSTALL-WITH-CODEX.md" "$KIT_DIR/INSTALL-WITH-CODEX.md"
cp -p "$PROJECT_DIR/INSTALL-WITH-CODEX.en.md" "$KIT_DIR/INSTALL-WITH-CODEX.en.md"
cp -p "$PROJECT_DIR/install/INSTALL-GUIDE.zh-CN.md" "$KIT_DIR/install/INSTALL-GUIDE.zh-CN.md"
cp -p "$PROJECT_DIR/install/INSTALL-GUIDE.en.md" "$KIT_DIR/install/INSTALL-GUIDE.en.md"
cp -p "$PROJECT_DIR/install/install-codex-kimi-bridge.command" "$KIT_DIR/install-codex-kimi-bridge.command"
cp -p "$PROJECT_DIR/install/start-codex-kimi-bridge.command" "$KIT_DIR/start-codex-kimi-bridge.command"
cp -p "$PROJECT_DIR/install/install-launchagent.command" "$KIT_DIR/install-launchagent.command"
cp -p "$PROJECT_DIR/install/uninstall-launchagent.command" "$KIT_DIR/uninstall-launchagent.command"
cp -p "$PROJECT_DIR/install/launchagent/io.github.rinranx.codex-kimi-bridge.plist" "$KIT_DIR/launchagent/"
cp -p "$PROJECT_DIR/install/templates/config-kimi-provider.toml" "$KIT_DIR/templates/"
cp -p "$PROJECT_DIR/install/templates/kimi_frontend.toml" "$KIT_DIR/templates/"
cp -p "$PROJECT_DIR/companion-skill/manage-codex-kimi-bridge/SKILL.md" "$KIT_DIR/companion-skill/manage-codex-kimi-bridge/SKILL.md"
cp -p "$PROJECT_DIR/companion-skill/manage-codex-kimi-bridge/agents/openai.yaml" "$KIT_DIR/companion-skill/manage-codex-kimi-bridge/agents/openai.yaml"

/usr/bin/lipo -create "$ARM_SOURCE" "$INTEL_SOURCE" -output "$KIT_DIR/bin/codex-kimi-bridge"
chmod 0755 "$KIT_DIR/bin/codex-kimi-bridge" "$KIT_DIR"/*.command
/usr/bin/codesign --force --sign - --timestamp=none "$KIT_DIR/bin/codex-kimi-bridge"
[[ "$($KIT_DIR/bin/codex-kimi-bridge --version)" == "$VERSION" ]]
(cd "$KIT_DIR" && shasum -a 256 bin/codex-kimi-bridge > BINARY-SHA256.txt)

KIT_ARCHIVE="$KIT_NAME.zip"
COPYFILE_DISABLE=1 /usr/bin/ditto --norsrc -c -k --keepParent "$KIT_DIR" "$OUTPUT_DIR/$KIT_ARCHIVE"
if /usr/bin/unzip -Z1 "$OUTPUT_DIR/$KIT_ARCHIVE" | grep -Eq '(^|/)(__MACOSX|\.DS_Store)(/|$)'; then
  echo "unwanted macOS metadata found in $KIT_ARCHIVE" >&2
  exit 1
fi

package_architecture() {
  local ARCH_LABEL="$1"
  local SOURCE_BINARY="$2"
  local PACKAGE_NAME="codex-kimi-bridge-macos-$ARCH_LABEL-$VERSION"
  local PACKAGE_DIR="$STAGE_DIR/$PACKAGE_NAME"

  mkdir -p "$PACKAGE_DIR/launchagent"
  copy_release_docs "$PACKAGE_DIR"
  cp -p "$PROJECT_DIR/install/INSTALL-GUIDE.zh-CN.md" "$PACKAGE_DIR/INSTALL-GUIDE.zh-CN.md"
  cp -p "$PROJECT_DIR/install/INSTALL-GUIDE.en.md" "$PACKAGE_DIR/INSTALL-GUIDE.en.md"
  cp -p "$PROJECT_DIR/install/start-codex-kimi-bridge.command" "$PACKAGE_DIR/start-codex-kimi-bridge.command"
  cp -p "$PROJECT_DIR/install/install-launchagent.command" "$PACKAGE_DIR/install-launchagent.command"
  cp -p "$PROJECT_DIR/install/uninstall-launchagent.command" "$PACKAGE_DIR/uninstall-launchagent.command"
  cp -p "$PROJECT_DIR/install/launchagent/io.github.rinranx.codex-kimi-bridge.plist" "$PACKAGE_DIR/launchagent/"
  cp -p "$SOURCE_BINARY" "$PACKAGE_DIR/codex-kimi-bridge"
  chmod 0755 "$PACKAGE_DIR/codex-kimi-bridge" "$PACKAGE_DIR"/*.command
  /usr/bin/codesign --force --sign - --timestamp=none "$PACKAGE_DIR/codex-kimi-bridge"
  [[ "$($PACKAGE_DIR/codex-kimi-bridge --version)" == "$VERSION" ]]
  (cd "$PACKAGE_DIR" && shasum -a 256 codex-kimi-bridge > BINARY-SHA256.txt)
  COPYFILE_DISABLE=1 /usr/bin/tar -czf "$OUTPUT_DIR/$PACKAGE_NAME.tar.gz" -C "$STAGE_DIR" "$PACKAGE_NAME"
}

package_architecture "arm64" "$ARM_SOURCE"
package_architecture "x86_64" "$INTEL_SOURCE"

mkdir -p "$STAGE_DIR/npm-cache"
NPM_ARCHIVE="$(npm_config_cache="$STAGE_DIR/npm-cache" npm pack --silent --pack-destination "$OUTPUT_DIR" "$PROJECT_DIR/node" | tail -n 1)"
if [[ "$NPM_ARCHIVE" != "codex-kimi-bridge-node-0.1.0.tgz" || ! -f "$OUTPUT_DIR/$NPM_ARCHIVE" ]]; then
  echo "unexpected Node fallback archive: $NPM_ARCHIVE" >&2
  exit 1
fi

ARM_ARCHIVE="codex-kimi-bridge-macos-arm64-$VERSION.tar.gz"
INTEL_ARCHIVE="codex-kimi-bridge-macos-x86_64-$VERSION.tar.gz"
(
  cd "$OUTPUT_DIR"
  shasum -a 256 "$KIT_ARCHIVE" > INSTALL-KIT-SHA256.txt
  shasum -a 256 "$KIT_ARCHIVE" "$ARM_ARCHIVE" "$INTEL_ARCHIVE" "$NPM_ARCHIVE" > SHA256SUMS.txt
  shasum -a 256 -c SHA256SUMS.txt
)

EXPECTED_COUNT=6
ACTUAL_COUNT="$(find "$OUTPUT_DIR" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d ' ')"
if [[ "$ACTUAL_COUNT" != "$EXPECTED_COUNT" ]]; then
  echo "expected $EXPECTED_COUNT release assets, found $ACTUAL_COUNT" >&2
  exit 1
fi

echo "Packaged immutable release assets in $OUTPUT_DIR:"
find "$OUTPUT_DIR" -mindepth 1 -maxdepth 1 -type f -exec basename {} \; | LC_ALL=C sort
