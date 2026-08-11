FROM --platform=linux/amd64 node:24.12.0-bookworm-slim@sha256:6d8047885b91084ceff824c02950be237dafcbfd3d1b6e69d49c919868e806be

ENV HOME=/nonexistent \
    NPM_CONFIG_UPDATE_NOTIFIER=false \
    NPM_CONFIG_FUND=false \
    NPM_CONFIG_AUDIT=false

WORKDIR /opt/participant
COPY package.json package-lock.json ./
RUN node -e 'const lock=require("./package-lock.json"); const bad=Object.entries(lock.packages).filter(([name,row]) => name && row.resolved && !(typeof row.integrity === "string" && row.integrity.startsWith("sha512-"))); if (bad.length) { console.error(bad.map(([name]) => name).join("\n")); process.exit(1); }' \
    && npm ci --ignore-scripts --omit=dev \
    && npm cache clean --force \
    && test "$(node --version)" = "v24.12.0" \
    && test "$(node -p 'JSON.parse(require("fs").readFileSync("node_modules/@earendil-works/pi-coding-agent/package.json")).version')" = "0.84.1"

COPY participant.mjs auth-preflight.mjs request-capture.mjs egress-broker.mjs ./
COPY LICENSE.pi-v0.84.1.base64 /tmp/pi-MIT.base64
RUN mkdir /licenses \
    && base64 --decode /tmp/pi-MIT.base64 > /licenses/pi-MIT.txt \
    && test "$(wc -c < /licenses/pi-MIT.txt)" = "1069" \
    && test "$(sha256sum /licenses/pi-MIT.txt | cut -d ' ' -f 1)" = "0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48" \
    && rm /tmp/pi-MIT.base64
RUN chown -R root:root /opt/participant \
    && chmod -R a-w /opt/participant \
    && mkdir /workspace \
    && chown root:root /workspace \
    && chmod 0555 /workspace

USER node
WORKDIR /workspace
ENTRYPOINT ["node", "/opt/participant/participant.mjs"]
