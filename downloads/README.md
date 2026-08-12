# Downloads moved to GitHub Releases

Built archives and checksums are no longer stored or overwritten on `main`.

Download the current release from [`v0.3.0`](https://github.com/rinranx/codex-kimi-bridge/releases/tag/v0.3.0). Every archive has a versioned filename and is attached to its matching Git tag and GitHub Release.

Available assets:

| File | Use |
| --- | --- |
| `codex-kimi-bridge-macos-install-kit-0.3.0.zip` | Recommended universal kit for Apple Silicon and Intel Macs |
| `codex-kimi-bridge-macos-arm64-0.3.0.tar.gz` | Apple Silicon standalone binary package |
| `codex-kimi-bridge-macos-x86_64-0.3.0.tar.gz` | Intel Mac standalone binary package |
| `codex-kimi-bridge-node-0.2.0.tgz` | Separately named Node fallback; not the default |
| `SHA256SUMS.txt` | Checksums for every packaged artifact |
| `INSTALL-KIT-SHA256.txt` | Checksum for the recommended universal kit |

The Rust binary is ad-hoc signed but not Apple-notarized. Read the root installation guide before bypassing a Gatekeeper warning.

If a published artifact needs a correction, publish a new version. Never replace an existing Release asset or move its tag.
