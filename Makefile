# Serenade and Rangular MCP developer targets.

include make/common.mk
include make/quality.mk
include make/docker.mk
include make/version.mk

.DEFAULT_GOAL := help
.PHONY: help

help:
	@echo "Serenade and Rangular MCP targets"
	@echo ""
	@echo "  build / release / check  build or check the workspace"
	@echo "  run                      start stdio MCP (ARGS='...')"
	@echo "  format / format-check / lint  formatting and strict Clippy"
	@echo "  test / doc / ci          tests, rustdoc and all quality gates"
	@echo "  audit / deny / machete / outdated  dependency checks"
	@echo "  coverage / coverage-summary / coverage-html / tarpaulin"
	@echo "  publish-check            verify the crate package without publishing"
	@echo "  refresh-docs             refresh embedded source snapshots"
	@echo "  version-show / version-bump-patch / version-bump-minor / version-bump-major"
	@echo "  version-set VERSION=X.Y.Z"
	@echo "  docker-build / docker-build-dev / docker-build-release"
	@echo "  docker-push-dev-hub / docker-push-dev-ghcr"
	@echo "  docker-push-release-hub / docker-push-release-ghcr"
	@echo "  docker-tag-latest / docker-push-latest-hub / docker-push-latest-ghcr"
	@echo ""
	@echo "Quality and version commands require the Cargo tools listed in docs/README.md."
