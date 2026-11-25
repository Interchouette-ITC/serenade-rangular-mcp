.PHONY: version-show version-bump-patch version-bump-minor version-bump-major version-set

APP_VERSION ?= $(shell awk '/^\[package\]/{pkg=1; next} /^\[/{pkg=0} pkg && /^version = /{gsub(/"/,"",$$3); print $$3; exit}' Cargo.toml)

version-show:
	@echo "Package version: $(APP_VERSION)"
	@echo "Release tag: v$(APP_VERSION)"

version-bump-patch:
	$(CARGO) set-version --package serenade-rangular-mcp --bump patch

version-bump-minor:
	$(CARGO) set-version --package serenade-rangular-mcp --bump minor

version-bump-major:
	$(CARGO) set-version --package serenade-rangular-mcp --bump major

version-set:
	@test -n "$(VERSION)" || { echo "Usage: make version-set VERSION=X.Y.Z" >&2; exit 1; }
	$(CARGO) set-version --package serenade-rangular-mcp "$(VERSION)"
