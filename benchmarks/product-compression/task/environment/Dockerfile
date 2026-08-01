FROM --platform=linux/amd64 alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce

# The task runtime has no package-network access. Harbor detects this exact
# official Codex installation and then owns execution, OAuth, and results.
RUN apk add --no-cache bash ca-certificates gcompat git jq libgcc nodejs npm ripgrep \
 && npm install --global @openai/codex@{{CODEX_VERSION}} \
 && codex --version
COPY frontier.bundle /opt/vela-input/frontier.bundle
COPY fixture.json answer.schema.json /opt/vela-input/
RUN git clone --quiet /opt/vela-input/frontier.bundle /workspace/frontier \
 && git -C /workspace/frontier checkout --quiet --detach "$(jq -r '.receiver.git_commit' /opt/vela-input/fixture.json)" \
 && git -C /workspace/frontier remote remove origin
{{VELA_INSTALL}}
WORKDIR /workspace/frontier
