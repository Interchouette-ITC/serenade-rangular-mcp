# Releasing Serenade and Rangular MCP

The current package version is in [Cargo.toml](../Cargo.toml). The
[release workflow](../.github/workflows/release.yml) runs when a GitHub Release
is published and supports manual reruns for a published tag. It validates the
tag against the package version and builds from that tag's commit.
Publication is not implied by this documentation.

## Preparation

1. Update the version with `make version-set VERSION=X.Y.Z` or a
   `make version-bump-*` target; these require `cargo-edit`. Update
   [CHANGELOG.md](CHANGELOG.md) for the version.
2. Refresh the corpus with `make refresh-docs` when needed. Review the
   resulting source revisions, hashes and parser version.
3. Verify locally:

   ```sh
   make version-show
   make ci
   make coverage
   make publish-check PUBLISH_CHECK_ARGS=--allow-dirty
   ```

4. Configure the `crates-io` environment with `CARGO_REGISTRY_TOKEN` for a
   stable release. Image jobs need the Docker Hub and personal and
   Interchouette GHCR credentials described in
   [docker/README.md](../docker/README.md). Codecov uses `CODECOV_TOKEN` when
   required by the repository's Codecov configuration.

## Release workflow

Publish a GitHub Release for `v<version>` to trigger the workflow.
Prerelease versions containing `-` must be marked as prereleases; versions
with build metadata (`+`) are rejected.

- The build job runs `make ci`, then builds and uploads the Linux x86_64
  executable and its SHA-256 file.
- Windows x86_64 and macOS Apple silicon executables and checksums are built
  and uploaded by the platform matrix.
- Images are built for Docker Hub and all three GHCR namespaces with the
  `<version>` tag. Stable releases also receive `latest`.
- Stable releases publish the crate to crates.io. The workflow checks the
  registry first and skips a version that is already published.

The platform assets, image and crate jobs run independently after the build
job; one can succeed even if another fails. The separate dev image workflow
only targets `:dev`.

## Resuming

Fix external failures such as missing credentials or registry availability,
then rerun the failed jobs or manually dispatch the current release workflow
with the published tag. Existing assets are replaced and an already-published
crate is skipped. Do not move an existing release tag; a fix to tagged code
requires a new version.
