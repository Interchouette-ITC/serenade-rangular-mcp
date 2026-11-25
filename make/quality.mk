.PHONY: build release check run format format-check lint test doc ci \
	machete outdated audit deny coverage coverage-summary coverage-html tarpaulin \
	publish-check refresh-docs

build:
	$(CARGO) build --locked --workspace

release:
	$(CARGO) build --release --locked --bin serenade-rangular-mcp

check:
	$(CARGO) check --locked --workspace --all-targets

run:
	$(CARGO) run --locked --bin serenade-rangular-mcp -- $(ARGS)

format:
	$(CARGO) fmt --all

format-check:
	$(CARGO) fmt --all --check

lint: format-check
	$(CARGO) clippy --locked --workspace --all-targets -- $(CLIPPY_FLAGS)

test:
	$(CARGO) test --locked --workspace --all-targets

doc:
	RUSTDOCFLAGS="$(RUSTDOCFLAGS)" $(CARGO) doc --locked --workspace --no-deps

machete:
	$(CARGO) machete

outdated:
	$(CARGO) outdated --workspace

audit:
	$(CARGO) audit

deny:
	$(CARGO) deny check

coverage:
	mkdir -p coverage
	$(CARGO) llvm-cov --locked --workspace --lcov --output-path coverage/lcov.info

coverage-summary:
	$(CARGO) llvm-cov --locked --workspace --summary-only

coverage-html:
	mkdir -p coverage
	$(CARGO) llvm-cov --locked --workspace --html --output-dir coverage/html

tarpaulin:
	mkdir -p coverage/tarpaulin
	$(CARGO) tarpaulin --locked --workspace --out Html --out Xml --output-dir coverage/tarpaulin

ci: lint test doc audit deny machete

publish-check:
	$(CARGO) package --locked $(PUBLISH_CHECK_ARGS)

refresh-docs:
	$(CARGO) run --locked -p refresh-docs
