FROM --platform=linux/amd64 alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce

RUN apk add --no-cache bash ca-certificates gcompat libgcc nodejs npm python3 ripgrep \
 && npm install --global @openai/codex@0.145.0 \
 && codex --version \
 && adduser -D -u 10001 -h /home/participant participant

COPY input /input
COPY answer.schema.json /input/answer.schema.json
RUN find /input -type d -exec chmod 0555 {} + \
 && find /input -type f -exec chmod 0444 {} + \
 && mkdir -p /workspace /logs/artifacts \
 && chown -R participant:participant /workspace /logs \
 && chmod 0555 /input

ENV HOME=/home/participant
USER participant
WORKDIR /workspace
