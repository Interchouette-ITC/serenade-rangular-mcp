ARG RUST_IMAGE=rust:slim-trixie
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot

FROM ${RUST_IMAGE} AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY tools ./tools
COPY corpus ./corpus
RUN cargo build --release --locked --bin serenade-rangular-mcp

FROM ${RUNTIME_IMAGE}
COPY --from=builder /app/target/release/serenade-rangular-mcp /usr/local/bin/serenade-rangular-mcp
ENV MCP_BIND=0.0.0.0 PORT=8765 MCP_ALLOWED_HOSTS=mcp.serenade.rs
EXPOSE 8765
ENTRYPOINT ["/usr/local/bin/serenade-rangular-mcp", "--transport", "http"]
