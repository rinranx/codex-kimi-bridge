# Downloads

The default release is Rust `codex-kimi-bridge 0.2.0-alpha.1`.

| File | Use |
| --- | --- |
| `codex-kimi-bridge-macos-install-kit-0.2.0-alpha.1.zip` | Recommended universal kit for Apple Silicon and Intel Macs |
| `codex-kimi-bridge-macos-arm64-0.2.0-alpha.1.tar.gz` | Apple Silicon standalone binary package |
| `codex-kimi-bridge-macos-x86_64-0.2.0-alpha.1.tar.gz` | Intel Mac standalone binary package |
| `node/codex-kimi-bridge-node-0.1.0.tgz` | Separately named Node fallback; not the default |

Verify the universal kit against `INSTALL-KIT-SHA256.txt`. Verify architecture packages and the Node fallback against `SHA256SUMS`.

The Rust alpha is ad-hoc signed but not Apple-notarized. Read the root installation guide before bypassing a Gatekeeper warning.

The universal kit also contains three startup choices: Codex-managed on demand, a visible Terminal launcher, and optional login-time LaunchAgent scripts.
