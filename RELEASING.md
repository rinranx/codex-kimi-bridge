# Release process

`codex-kimi-bridge` uses versioned Git tags and GitHub Releases. Built archives do not live on `main`, and a published asset is never overwritten.

## One-time repository setting

Before the first release, enable **Release immutability** in the repository's Settings → General → Releases section. The setting must be enabled before publishing; it does not retroactively protect older releases.

## Publishing a version

1. Choose a new SemVer version. Never reuse an existing version or tag.
2. Update the Rust crate, installer checks, tests, documentation, and fixed download links to that version.
3. For protocol changes, compare the Rust and Node translations of `compat/responses-request.json`. For signed handoff changes, require offline tests for hook capture, configuration merge/uninstall, HMAC verification, tampering, expiry, recipient binding, empty-payload failure, and explicit marked recursive handoff. Then complete [`compat/AGENT-MESSAGE-INTEGRATION.md`](compat/AGENT-MESSAGE-INTEGRATION.md) with explicit user consent; the real Desktop check consumes Kimi quota.
4. Add a new `release-manifests/v<version>.json` and matching `.md` release-notes file.
5. Push the source commit to `main` only after local tests pass and Release immutability is enabled.
6. The `Publish immutable release` workflow tests both implementations, builds both macOS architectures, creates checksums, creates an annotated tag, uploads assets to a draft release, and compares every server-reported asset digest and size with the local build before publishing. The manifest decides whether the final Release is stable or a prerelease.
7. After GitHub reports the published Release as immutable, the workflow downloads every public asset, compares it byte-for-byte with the local build, and rechecks `SHA256SUMS.txt`.

The workflow never uses `--clobber`. If a released artifact is wrong, increment the version and publish another Release. Do not move a tag, delete an asset, or replace a filename attached to an existing version.

## Release assets

- `codex-kimi-bridge-macos-install-kit-<version>.zip`
- `codex-kimi-bridge-macos-arm64-<version>.tar.gz`
- `codex-kimi-bridge-macos-x86_64-<version>.tar.gz`
- `codex-kimi-bridge-node-<node-version>.tgz`
- `INSTALL-KIT-SHA256.txt`
- `SHA256SUMS.txt`

The source archives shown by GitHub are generated from the annotated tag.
