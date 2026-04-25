# vela-workbench: live read-only API for the BBB frontier.
#
# Runs `vela serve` against frontiers/bbb-alzheimer.json. Exposes
# `/api/{frontier,findings,events,queue,...}`. The Workbench UI lives
# in the Astro site at vela-site/.../workbench and proxies through
# nginx to this API as same-origin.
#
# Read-only by design — the substrate doesn't accept writes via this
# surface; the queue endpoint stages drafts that the CLI signs.

FROM rust:1.89-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Workspace context: copy everything needed to build the CLI binary
# and the web/frontier assets it serves at runtime.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY frontiers ./frontiers

RUN cargo build --release -p vela-protocol --bin vela

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 vela

WORKDIR /app
COPY --from=builder /build/target/release/vela /usr/local/bin/vela
COPY --from=builder /build/frontiers ./frontiers

USER vela
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/vela", "serve", "--http", "8080", "frontiers/bbb-alzheimer.json"]
