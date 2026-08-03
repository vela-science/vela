FROM --platform=linux/amd64 ubuntu@sha256:52df9b1ee71626e0088f7d400d5c6b5f7bb916f8f0c82b474289a4ece6cf3faf

ARG DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.elan/bin:${PATH}"

RUN apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates curl git jq python3 zstd \
 && rm -rf /var/lib/apt/lists/*
RUN curl --fail --silent --show-error --location \
      https://raw.githubusercontent.com/leanprover/elan/v4.2.1/elan-init.sh \
      --output /tmp/elan-init.sh \
 && echo "4bacca9502cb89736fe63d2685abc2947cfbf34dc87673504f1bb4c43eda9264  /tmp/elan-init.sh" | sha256sum --check \
 && sh /tmp/elan-init.sh -y --default-toolchain none \
 && rm /tmp/elan-init.sh

COPY formal-conjectures.bundle /opt/vela-input/formal-conjectures.bundle
RUN git clone --quiet /opt/vela-input/formal-conjectures.bundle /workspace/formal-conjectures \
 && git -C /workspace/formal-conjectures checkout --quiet --detach {{FORMAL_COMMIT}} \
 && git -C /workspace/formal-conjectures remote remove origin
RUN cd /workspace/formal-conjectures \
 && before="$(sha256sum lake-manifest.json | cut -d' ' -f1)" \
 && lake update \
 && test "$(sha256sum lake-manifest.json | cut -d' ' -f1)" = "$before" \
 && lake exe cache get \
 && lake build FormalConjecturesUtil

COPY fixture.json verify.py test.sh /tests/
RUN chmod 0555 /tests/test.sh /tests/verify.py
WORKDIR /workspace/formal-conjectures
