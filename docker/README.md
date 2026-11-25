# HTTP container

Build and run locally:

```sh
make docker-build
docker run --rm -p 127.0.0.1:8765:8765 serenade-rangular-mcp
```

The MCP endpoint is `http://127.0.0.1:8765/mcp` and `/health` is the health
check. The HTTP transport is read-only. For a public deployment, terminate
HTTPS at a reverse proxy and set `MCP_ALLOWED_HOSTS` to the actual hostnames.
The image defaults to `mcp.serenade.rs`; override this for other deployments.
The server rejects browser Origin headers. See the [MCP guide](../docs/README.md)
for transport and security details.

The release workflow targets version tags on
`interchouette/serenade-rangular-mcp` and
`ghcr.io/interchouette-itc/serenade-rangular-mcp`. Stable releases also target
`latest`; the separate dev workflow targets only `dev`. The workflows require
Docker Hub's `DOCKER_USERNAME` and `DOCKER_PASSWORD` secrets and GHCR package
write access. These destinations must not be presented as published until
their workflows succeed and the images are verified.

`make docker-build-dev` tags both registries with `dev`;
`make docker-build-release` tags both with the Cargo version prefixed by `v`.
Use the corresponding per-registry push targets from `make help` after
authenticating. `make docker-tag-latest` and the `docker-push-latest-*`
targets are for stable releases only.
