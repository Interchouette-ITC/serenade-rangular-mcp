# HTTP container

Build and run locally:

```sh
make docker-build
docker run --rm -p 127.0.0.1:8765:8765 serenade-rangular-mcp
```

The MCP endpoint is `http://127.0.0.1:8765/`; `/health` is the health check.
The HTTP transport is read-only. For a public deployment, terminate HTTPS at a
reverse proxy and set `MCP_ALLOWED_HOSTS` to the actual hostnames.
The image defaults to `mcp.serenade.rs`; override this for other deployments.
The server rejects browser Origin headers. See the [MCP guide](../docs/README.md)
for transport and security details.

Image destinations:

| Registry | Image |
| --- | --- |
| Docker Hub | `interchouette/serenade-rangular-mcp` |
| Personal GHCR | `ghcr.io/groussac/serenade-rangular-mcp` |
| Interchouette GHCR | `ghcr.io/interchouette/serenade-rangular-mcp` |
| Interchouette-ITC GHCR | `ghcr.io/interchouette-itc/serenade-rangular-mcp` |

The dev workflow builds and pushes `:dev`. The release workflow builds and
pushes `:<version>`; stable releases also push `:latest`. A prerelease does
not update `:latest`. The version is the Cargo version without the `v`
prefix used on GitHub release tags.

Workflows require Docker Hub's `DOCKER_USERNAME` and `DOCKER_PASSWORD`,
personal GHCR's `GHCR_USERNAME` and `GHCR_PAT`, and Interchouette GHCR's
`GHCR_USERNAME_ITC` and `GHCR_PAT_ITC` secrets. The respective accounts need
permission to publish each package. A configured destination is not proof of
a successful push or public visibility.

`make docker-build-dev` and `make docker-build-release` build and tag all four
images. Use the per-registry push targets from `make help` after authenticating
to each registry. `make docker-tag-latest` and the `docker-push-latest-*`
targets are for stable releases only.
