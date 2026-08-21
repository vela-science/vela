FROM node:24.8.0-bookworm-slim@sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85

RUN npm install --global --omit=dev --ignore-scripts @openai/codex@0.149.0
WORKDIR /opt/vela-runner
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --ignore-scripts && npm cache clean --force
COPY run-once.mjs strict-config.mjs strict-preflight.mjs ./

RUN useradd --create-home --uid 10001 participant \
    && mkdir -p /work /input /permit /evidence /codex-home \
    && chown -R participant:participant /work /input /permit /evidence /codex-home

USER participant
ENV CODEX_HOME=/codex-home
ENTRYPOINT ["node", "/opt/vela-runner/run-once.mjs"]
