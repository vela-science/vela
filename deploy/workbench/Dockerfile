# vela-workbench: live Workbench surface for the BBB frontier.
#
# Runs `vela serve --workbench` against frontiers/bbb-alzheimer.json,
# which exposes `/api/{frontier,findings,events,...}` plus serves the
# `web/` directory (marketing landing + workbench previews) from the
# same origin. Read-only by design — the substrate doesn't accept
# writes via this surface.

FROM rust:1.89-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Workspace context: copy everything needed to build the CLI binary
# and the web/frontier assets it serves at runtime.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY web ./web
COPY assets ./assets
COPY frontiers ./frontiers

RUN cargo build --release -p vela-protocol --bin vela

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 vela

WORKDIR /app
COPY --from=builder /build/target/release/vela /usr/local/bin/vela
COPY --from=builder /build/web ./web
COPY --from=builder /build/assets ./assets
COPY --from=builder /build/frontiers ./frontiers

USER vela
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/vela", "serve", "--http", "8080", "--workbench", "frontiers/bbb-alzheimer.json"]
