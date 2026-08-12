# Release process

`codex-kimi-bridge` uses versioned Git tags and GitHub Releases. Built archives do not live on `main`, and a published asset is never overwritten.

## One-time repository setting

Before the first release, enable **Release immutability** in the repository's Settings → General → Releases section. The setting must be enabled before publishing; it does not retroactively protect older releases.

## Publishing a version

1. Choose a new SemVer version. Never reuse an existing version or tag.
2. Update the Rust crate, installer checks, tests, documentation, and fixed download links to that version.
3. Add a new `release-manifests/v<version>.json` and matching `.md` release-notes file.
4. Push the source commit to `main` only after local tests pass and Release immutability is enabled.
5. The `Publish immutable release` workflow tests both implementations, builds both macOS architectures, creates checksums, creates an annotated tag, uploads assets to a draft prerelease, downloads and byte-checks every draft asset, and only then publishes it.
6. Confirm that the workflow reports the published Release as immutable and that every asset digest and size matches the local build output.

The workflow never uses `--clobber`. If a released artifact is wrong, increment the version and publish another Release. Do not move a tag, delete an asset, or replace a filename attached to an existing version.

## Release assets

- `codex-kimi-bridge-macos-install-kit-<version>.zip`
- `codex-kimi-bridge-macos-arm64-<version>.tar.gz`
- `codex-kimi-bridge-macos-x86_64-<version>.tar.gz`
- `codex-kimi-bridge-node-0.1.0.tgz`
- `INSTALL-KIT-SHA256.txt`
- `SHA256SUMS.txt`

The source archives shown by GitHub are generated from the annotated tag.
