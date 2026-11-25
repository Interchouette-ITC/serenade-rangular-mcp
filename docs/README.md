# Serenade and Rangular MCP

Source-backed documentation and Rangular template diagnostics for coding
agents. One binary supports stdio and Streamable HTTP. Documentation is
embedded in the binary; the website, source checkouts and network are not
needed at runtime.

## Build and install

```sh
make ci
cargo install --locked --path . --bin serenade-rangular-mcp
```

The binary uses the `serenade`, `rangular` or `all` profile (`all` by default).
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
- `search_docs`: search titles, headings and source text.
- `get_doc`: read a document or a section; large documents return section IDs.
  If a section is still too large, pass `offset: 0` and follow `next_offset`.
- `validate_rangular_template`: parse component or head template text and
  report up to 100 RANG diagnostics, with the total count and a truncation flag.
  This is not a full AOT compile or application build.
- MCP resources expose the same embedded documents as `docs://<document-id>`.
  Some MCP clients do not expose resources; use `get_doc` instead.

Each result identifies the product, documented version, source revision and
original source path. The current Rangular corpus and parser use the matching
`1.1.0` release. Serenade documents a selected development revision, not
necessarily the latest published release. Agents run the existing `serenade`
CLI, `serenade-trunk` and Cargo themselves under their normal permissions.

## Refresh the embedded documentation

Update the exact `rangular-parser` crates.io version in `Cargo.toml` to the
desired release first. Then supply full source commit IDs:

```sh
make refresh-docs \
  SERENADE_ROOT=/path/to/serenade SERENADE_REV=<serenade-commit> \
  RANGULAR_ROOT=/path/to/rangular RANGULAR_REV=<rangular-release-commit>
make ci
```

The Rust refresh tool reads the selected commits, not the current contents of
the source checkouts. It checks Rangular's tag and package version against
the locked crates.io parser, expression and host packages' published source
revisions. Dependencies must already be available locally (`cargo fetch --locked`
if needed). The manifest records source paths, versions, revisions
and content hashes. Builds verify those hashes and reject mismatched parser
versions. Commit the refreshed corpus with the implementation; it never
updates itself.

The embedded upstream content is sourced from Serenade and Rangular, each
distributed under Apache-2.0. Original source locations and revisions are
preserved in the corpus manifest.
