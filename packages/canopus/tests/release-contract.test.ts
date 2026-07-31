import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

test("CI pins one reproducible Vela and Codex composition without defining runtime compatibility", async () => {
  const [workflow, lockText] = await Promise.all([
    readFile(new URL("../../../../.github/workflows/agent-ci.yml", import.meta.url), "utf8"),
    readFile(new URL("../../toolchain.lock.json", import.meta.url), "utf8"),
  ]);
  const lock = JSON.parse(lockText) as {
    vela?: { version?: string; tag?: string; source_commit?: string; assets?: object };
    codex?: { version?: string };
  };

  assert.match(lock.vela?.version ?? "", /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u);
  assert.equal(lock.vela?.tag, `v${lock.vela?.version}`);
  assert.match(lock.codex?.version ?? "", /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u);
  assert.match(lock.vela?.source_commit ?? "", /^[0-9a-f]{40}$/u);
  assert.equal(Object.keys(lock.vela?.assets ?? {}).length, 3);
  assert.match(workflow, /tooling\/export-toolchain-env\.mjs/u);
  assert.doesNotMatch(workflow, /releases\/download\/v0\.\d+\.\d+/u);
  assert.doesNotMatch(workflow, /archive_sha256:/u);
  assert.doesNotMatch(workflow, /binary_sha256:/u);
});

test("agent CI validates Protocol packaging without packaging the private Agent helper", async () => {
  const workflow = await readFile(
    new URL("../../../../.github/workflows/agent-ci.yml", import.meta.url),
    "utf8",
  );

  assert.equal(
    workflow.match(/bun pm pack --cwd packages\/protocol/gu)?.length,
    2,
    "Unix and Windows CI must validate the local Protocol archive",
  );
  assert.equal(
    workflow.match(/publish-protocol\.mjs check/gu)?.length,
    2,
    "Unix and Windows CI must validate the exact Protocol archive without publishing",
  );
  assert.doesNotMatch(workflow, /bun pm pack --cwd packages\/canopus/u);
  assert.doesNotMatch(workflow, /vela-science-canopus|canopus-install/u);
  assert.doesNotMatch(workflow, /mission validate|profile validate/u);
  assert.match(workflow, /- "\.github\/release\/\*\*"/u);
  assert.match(workflow, /- "\.github\/workflows\/release\.yml"/u);
  assert.doesNotMatch(
    workflow,
    /npm install[^\n]*@vela-science\/protocol/u,
    "CI must not race a just-published Protocol version in the registry",
  );
});

test("one Vela release publishes only the public Protocol package", async () => {
  const workflow = await readFile(
    new URL("../../../../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );

  for (const contract of [
    "- \"v*.*.*\"",
    "environment: npm",
    "id-token: write",
    "actions/attest-build-provenance@",
    "gh attestation verify",
    "--signer-workflow",
    "--source-ref",
    "--source-digest",
    "--deny-self-hosted-runners",
    "bun pm pack --cwd packages/protocol",
    "Smoke the exact packed Protocol package",
    "publish-protocol.mjs check",
    "publish-protocol.mjs --execute",
    "protocolDigest",
    "sha256At",
    "https://slsa.dev/provenance/v1",
  ]) {
    assert.match(workflow, new RegExp(contract.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&"), "u"));
  }
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/u);
  assert.doesNotMatch(workflow, /bun pm pack --cwd packages\/canopus/u);
  assert.doesNotMatch(workflow, /product-v/u);
});

test("current Agent helper is private and historical Canopus replay remains linked", async () => {
  const [packageText, readme] = await Promise.all([
    readFile(new URL("../../package.json", import.meta.url), "utf8"),
    readFile(new URL("../../README.md", import.meta.url), "utf8"),
  ]);
  const packageJson = JSON.parse(packageText) as {
    files?: string[];
    private?: boolean;
    name?: string;
    bin?: Record<string, string>;
    exports?: Record<string, unknown>;
    publishConfig?: Record<string, unknown>;
    scripts?: Record<string, string>;
    version?: string;
  };
  assert.equal(packageJson.version, "0.0.0");
  assert.equal(packageJson.name, "@vela-science/agent-internal");
  assert.equal(
    packageJson.private,
    true,
    "the private Agent helper must not be publishable",
  );
  assert.equal(packageJson.bin, undefined);
  assert.equal(packageJson.exports, undefined);
  assert.equal(packageJson.files, undefined);
  assert.equal(packageJson.publishConfig, undefined);
  assert.equal(packageJson.scripts?.prepack, undefined);
  assert.equal(packageJson.scripts?.["pack:check"], undefined);
  await assert.rejects(
    access(new URL("../src/cli.js", import.meta.url)),
    /ENOENT/u,
    "a clean current build must not emit a standalone Canopus CLI",
  );
  assert.match(
    readme,
    /retired public product is frozen as Canopus `0\.8\.0`[\s\S]+product-v0\.8\.0/u,
  );
  assert.match(readme, /A Run is nonmutating/u);
  assert.match(readme, /only canonical `vela submit` registers/iu);
  assert.doesNotMatch(readme, /canopus submit\b/u);
  assert.doesNotMatch(readme, /canopus land|canopus inspect|canopus withdraw/u);
  assert.match(
    readme,
    /github\.com\/vela-science\/vela-research-harness\/blob\/v0\.6\.5\/BUILD_WEEK\.md/u,
  );
});
