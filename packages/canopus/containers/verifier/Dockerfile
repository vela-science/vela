FROM python@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d

LABEL org.opencontainers.image.source="https://github.com/vela-science/vela-research-harness"
LABEL org.opencontainers.image.description="Network-denied Canopus verifier runtime"
LABEL org.opencontainers.image.licenses="Apache-2.0 OR MIT"

RUN mkdir -p /input /capsule /artifacts \
    && touch /capsule/verifier \
    && for index in 0 1 2 3 4 5 6 7 8 9; do touch "/artifacts/$index"; done \
    && chmod 0555 /input /capsule /artifacts \
    && chmod 0444 /capsule/verifier /artifacts/*

ENTRYPOINT []
