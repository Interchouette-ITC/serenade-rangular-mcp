CARGO ?= cargo +stable
SERENADE_ROOT ?= ../serenade
RANGULAR_ROOT ?= ../rangular
SERENADE_REV ?=
RANGULAR_REV ?=

.PHONY: build lint test doc ci refresh-docs docker-build

build:
	$(CARGO) build --locked

lint:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --locked --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --locked --workspace --all-targets

doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --locked --workspace --no-deps

ci: lint test doc

refresh-docs:
	$(CARGO) run --locked -p refresh-docs -- --serenade-root "$(SERENADE_ROOT)" \
		--serenade-revision "$(SERENADE_REV)" --rangular-root "$(RANGULAR_ROOT)" \
		--rangular-revision "$(RANGULAR_REV)"

docker-build:
	docker build -f docker/Dockerfile -t serenade-rangular-mcp .
