FROM golang:1.25-bookworm@sha256:ea341baa9bd5ba6784f6d7161ace70544349a6242d54d34a0fbfd2c4d51c9d58 AS landrun-builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
RUN git clone https://github.com/Zouuup/landrun.git /src/landrun \
    && git -C /src/landrun checkout --detach 811cfff51ceaf3d9843708aa6d22e9b84ccac8b4 \
    && cd /src/landrun \
    && go build -trimpath -o /out/landrun cmd/landrun/main.go

FROM rust:1.89-bookworm@sha256:948f9b08a66e7fe01b03a98ef1c7568292e07ec2e4fe90d88c07bb14563c84ff AS nanoda-builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
RUN git clone https://github.com/ammkrn/nanoda_lib.git /src/nanoda \
    && git -C /src/nanoda checkout --detach ddfac2bf5a7b56cb46e141494427ff3dd55963c7 \
    && cargo build --release --locked --manifest-path /src/nanoda/Cargo.toml

FROM ubuntu:24.04@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        libgmp-dev \
        zstd \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 astra

COPY --from=landrun-builder /out/landrun /usr/local/bin/landrun
COPY --from=nanoda-builder /src/nanoda/target/release/nanoda_bin /usr/local/bin/nanoda_bin
COPY lean4export-wrapper.sh /usr/local/bin/lean4export-wrapper
COPY replay-profiles.sh /usr/local/bin/replay-profiles

USER 10001:10001
ENV HOME=/home/astra
ENV PATH=/home/astra/.elan/bin:/usr/local/bin:/usr/bin:/bin
WORKDIR /work

RUN git clone --no-checkout --filter=blob:none https://github.com/openai/ten-proofs.git . \
    && git fetch --quiet --no-tags origin 29362184c2b698c1b279bc85b3957ee813646c63 \
    && git checkout --detach FETCH_HEAD \
    && curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
        https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh \
        | sh -s -- -y --default-toolchain none \
    && lake exe cache get \
    && lake build All lean4export comparator

ENV COMPARATOR_LANDRUN=/usr/local/bin/landrun
ENV COMPARATOR_LEAN4EXPORT=/usr/local/bin/lean4export-wrapper
ENV COMPARATOR_NANODA=/usr/local/bin/nanoda_bin

ENTRYPOINT ["/usr/local/bin/replay-profiles"]
