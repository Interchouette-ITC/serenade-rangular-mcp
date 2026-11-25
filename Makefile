CARGO ?= cargo +stable
.PHONY: build lint test doc ci publish-check refresh-docs docker-build

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

publish-check:
	$(CARGO) package --locked $(PUBLISH_CHECK_ARGS)

refresh-docs:
	$(CARGO) run --locked -p refresh-docs

docker-build:
	docker build -f docker/Dockerfile -t serenade-rangular-mcp .
