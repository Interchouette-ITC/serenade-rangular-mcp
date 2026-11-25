# Serenade and Rangular MCP

<p align="center">
  <a href="https://github.com/Interchouette-ITC/serenade-rangular-mcp/actions/workflows/ci.yml"><img src="https://github.com/Interchouette-ITC/serenade-rangular-mcp/actions/workflows/ci.yml/badge.svg?branch=dev" alt="CI" /></a>
  <a href="https://codecov.io/gh/Interchouette-ITC/serenade-rangular-mcp"><img src="https://codecov.io/gh/Interchouette-ITC/serenade-rangular-mcp/branch/dev/graph/badge.svg" alt="codecov" /></a>
</p>

Source-backed documentation and Rangular template diagnostics for coding
agents. One binary supports stdio and Streamable HTTP. Documentation is
embedded in the binary; the website, source checkouts and network are not
needed at runtime.

## Install

```sh
cargo install --locked serenade-rangular-mcp
```

Maintainers can verify the package without uploading it using `make publish-check`
(`PUBLISH_CHECK_ARGS=--allow-dirty` for local changes). The release workflow
is configured to build version-labelled Linux, Windows and macOS binaries and checksums for
prereleases and stable releases, and to publish version-tagged GHCR and Docker
Hub images for both, and `latest` and the crate only for stable releases.
Crate publication requires the `CARGO_REGISTRY_TOKEN` secret in the
`crates-io` environment. To retry an existing published release, dispatch the
current workflow with the published tag as its input. The separate
dev workflow targets only `:dev`. Images target Docker Hub and the personal,
Interchouette and Interchouette-ITC GHCR namespaces. See
[Docker image guidance](../docker/README.md) for registry secrets and Make targets.

The binary uses the `serenade`, `rangular` or `all` profile (`all` by default).
Local stdio exposes project actions: they can create files and run Cargo,
including build scripts and tests. Use an MCP client that asks for approval.
For a stdio client:

```json
{
  "servers": {
    "serenade-rangular": {
      "type": "stdio",
      "command": "serenade-rangular-mcp",
      "args": ["--profile", "all"]
    }
  }
}
```

## HTTP

```sh
serenade-rangular-mcp --transport http --port 8765
```

The endpoint is `http://127.0.0.1:8765/mcp`; `/health` is the health check.
For a public deployment, set `MCP_BIND=0.0.0.0`, `PORT` and
`MCP_ALLOWED_HOSTS` (comma-separated hostnames). The server rejects browser
Origin headers and uses no authentication. It only serves embedded public
content and parses supplied template text; it cannot access a caller's files
or execute commands. Use HTTPS at the reverse proxy.

Render can eventually host the service at `mcp.serenade.rs`. It is not
deployed by this repository. A sleeping service can take roughly 15 seconds to
wake: allow sufficient client connection/request timeout and reconnect after
a restart where the client supports it.

## Tools and resources

- `list_docs`: browse by product or group, with pagination.
- `search_docs`: search paths, titles, headings and source text.
- `get_doc`: read a document or a section; large documents return section IDs.
  If a section is still too large, pass `offset: 0` and follow `next_offset`.
- `get_coding_guide`: actionable, source-linked workflows for app scaffolding,
  HTML, SPA, SSG, Universal, Rangular components and combining both projects.
- `inspect_cargo_manifest`: inspect supplied manifest text for product dependencies,
  aliases, features, workspace declarations and target-specific dependencies.
  It does not read files or resolve Cargo's dependency graph.
- `scaffold_serenade_app` (stdio only): run the installed `serenade new` CLI
  in an existing absolute parent directory without overwriting a destination.
- `run_cargo_task` (stdio only): run `cargo check`, `build` or `test` in an
  absolute project directory, optionally selecting a package and features.
  Cargo can execute project-owned build scripts and tests and download dependencies.
- `validate_rangular_template`: parse component or head template text and
  report up to 100 RANG diagnostics, with the total count and a truncation flag.
  It uses the published crates.io parser, which may differ from the development
  docs, and reports both versions. This is not a full AOT compile or application build.
- MCP resources expose the same embedded documents as `docs://<document-id>`.
  Some MCP clients do not expose resources; use `get_doc` instead.

Each result identifies the product, documented version, source revision and
original source path. Both products' documentation tracks their development
branches and may describe unreleased changes. The corpus records the sources
embedded when it was last refreshed; it does not fetch updates at runtime.
It includes framework and template documentation, authored Astro pages and
crate READMEs, excluding repository administration such as contribution,
release and security-reporting policies. Astro's generated guide pages come from the included
`docs-dev` sources; the crate catalog is represented by the crate READMEs.
Agents can use the stdio actions for scaffolding and Cargo tasks, or run the
existing `serenade` CLI, `serenade-trunk` and Cargo themselves under their
normal permissions.
The corpus includes text source, configuration and guides for the Serenade
HTML, SPA, SSG and Universal examples and the Rangular Leptos and Tauri demos.
Example Cargo manifests and binary assets are not embedded. Use the original
repository links returned with results to obtain complete runnable projects;
these examples depend on their source workspaces. For a standalone application,
start with the installation and recipes guides and use published crates.

## Refresh the embedded documentation

Refresh fetches the `dev` branches of Serenade and Rangular. It needs
`curl` and network access, but no source checkouts or manual revisions:

```sh
make refresh-docs
make ci
```

The tool automatically updates `rangular-parser` and the lockfile to match the
latest Rangular release, then verifies that the parser and its expression and
host dependencies were published from that release's revision. This released
parser is independent of the development documentation.
The manifest records source paths, versions,
revisions and content hashes. Builds verify those hashes; installed binaries
do not update themselves.
Git preserves the embedded files' original bytes on every platform so Windows
checkout line endings do not invalidate those hashes.
The embedded agent guide has a local correction marked in its result metadata.
Refresh rejects the old checkout-based guidance until the corrected upstream
guide is available on the selected development branch.

The embedded upstream content is sourced from Serenade and Rangular, each
distributed under Apache-2.0. Original source locations and revisions are
preserved in the corpus manifest.

## Development checks

Run `make help` for build, quality, image and version targets. `make ci` runs
format checking, strict Clippy (`all`, `pedantic` and `nursery`), tests,
rustdoc, audit, deny and machete. Install `cargo-audit`, `cargo-deny` and
`cargo-machete` for that gate. `make outdated`, coverage and version targets
also require `cargo-outdated`, `cargo-llvm-cov` (plus `llvm-tools-preview`),
`cargo-tarpaulin` or `cargo-edit`, respectively.

The dependency policy records the `chrono` unmaintained advisory: it is
transitive through rmcp and schemars and has no compatible maintained release.
`make audit` continues to report this warning.

CI also generates an LCOV report with `make coverage` and uploads it to
Codecov. Upload errors do not fail CI, matching Serenade's CI policy.

`make doc` generates rustdoc locally at
`target/doc/serenade_rangular_mcp/index.html`. After CI succeeds on a push to
`dev`, the workflow deploys rustdoc to `gh-pages`. GitHub Pages must be
configured to serve that branch; the hosted documentation is not available
until the deployment and Pages configuration succeed.

## Project documentation

- [Changelog](CHANGELOG.md)
- [Releasing](RELEASING.md)
- [Contributing](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security policy](SECURITY.md)
