# Contributing to Serenade and Rangular MCP

Thank you for improving the documentation MCP. Keep its embedded sources,
provenance and transport security consistent with the behavior described in
[README.md](README.md).

## Before opening a PR

1. Run `make ci` and add tests for behavior you change.
2. Keep one concern per PR and document public behavior.
3. Use conventional commits and the [PR template](pull_request_template.md).

## Toolchain and quality bar

The package requires Rust 1.100 or newer. The Make targets use stable Rust.
Install `rustfmt`, `clippy`, `cargo-audit`, `cargo-deny` and `cargo-machete`
for `make ci`.

| Gate | Command |
| --- | --- |
| Format and strict Clippy | `make lint` |
| Workspace tests | `make test` |
| Rustdoc | `make doc` |
| Audit, deny and unused dependencies | `make audit`, `make deny`, `make machete` |
| Full local gate | `make ci` |
| LCOV coverage | `make coverage` |
| Package verification without upload | `make publish-check PUBLISH_CHECK_ARGS=--allow-dirty` |

Clippy denies warnings and the `all`, `pedantic` and `nursery` groups.
Coverage requires `cargo-llvm-cov` and `llvm-tools-preview`. See `make help`
for other targets and their prerequisites in [README.md](README.md).
`make doc` writes the local API documentation to
`target/doc/serenade_rangular_mcp/index.html`; CI deploys it to `gh-pages`
after a successful `dev` push.

## Documentation and releases

Use English for documentation and keep source revisions and hashes accurate.
See [README.md](README.md) for corpus refresh and [RELEASING.md](RELEASING.md)
for release preparation. Report security issues as described in
[SECURITY.md](SECURITY.md); community standards are in
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
