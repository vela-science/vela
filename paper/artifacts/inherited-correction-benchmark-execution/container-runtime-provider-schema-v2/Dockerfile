ARG SOURCE_DATE_EPOCH=1757289600

FROM node:24.8.0-bookworm-slim@sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85 AS trust-source

ARG CA_CERTIFICATES_VERSION=20250419~deb12u1
ARG CA_CERTIFICATES_DEB_SHA256=62b08a77d985d4253894b1f69aebda5925034ca4e294add364167fad8cb64a44
RUN apt-get update \
    && cd /tmp \
    && apt-get download "ca-certificates=${CA_CERTIFICATES_VERSION}" \
    && echo "${CA_CERTIFICATES_DEB_SHA256}  ca-certificates_${CA_CERTIFICATES_VERSION}_all.deb" | sha256sum -c - \
    && mkdir -p /trust-source /trust-bundle \
    && dpkg-deb --extract "ca-certificates_${CA_CERTIFICATES_VERSION}_all.deb" /trust-source \
    && find /trust-source/usr/share/ca-certificates/mozilla -type f -name '*.crt' -print \
       | LC_ALL=C sort \
       | xargs cat > /trust-bundle/ca-certificates.crt \
    && test -s /trust-bundle/ca-certificates.crt

FROM node:24.8.0-bookworm-slim@sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85

COPY --from=trust-source /trust-bundle/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

RUN npm install --global --omit=dev --ignore-scripts @openai/codex@0.149.0 \
    && npm cache clean --force \
    && rm -rf /root/.npm /tmp/node-compile-cache
WORKDIR /opt/vela-runner
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --ignore-scripts \
    && npm cache clean --force \
    && rm -rf /root/.npm /tmp/node-compile-cache
COPY run-once.mjs provider-schema-preflight.mjs schema-preflight.mjs strict-config.mjs strict-preflight.mjs trust-preflight.mjs ./
COPY schema-fixtures ./schema-fixtures

RUN useradd --create-home --uid 10001 participant \
    && mkdir -p /work /input /permit /evidence /codex-home \
    && chown -R participant:participant /work /input /permit /evidence /codex-home

USER participant
ENV CODEX_HOME=/codex-home
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENTRYPOINT ["node", "/opt/vela-runner/run-once.mjs"]
