import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("CI pins one reproducible Vela and Codex composition without defining runtime compatibility", async () => {
  const [workflow, lockText] = await Promise.all([
    readFile(new URL("../../../../.github/workflows/product-ci.yml", import.meta.url), "utf8"),
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

test("installed-package smoke validates one synthetic generic Mission", async () => {
  const workflow = await readFile(
    new URL("../../../../.github/workflows/product-ci.yml", import.meta.url),
    "utf8",
  );
  const fixture =
    /packages[\\/]canopus[\\/]tests[\\/]fixtures[\\/]generic-mission[\\/]mission\.json/gu;

  assert.equal(
    workflow.match(fixture)?.length,
    2,
    "Unix and Windows installed-package smoke must validate the generic Mission",
  );
  assert.equal(workflow.match(/mission validate/gu)?.length, 2);
  assert.doesNotMatch(
    workflow,
    /profile validate erdos1056|profile validate formal-erdos/u,
    "installed-package smoke must not depend on a domain profile",
  );
  assert.equal(
    workflow.match(/bun pm pack --cwd packages\/protocol/gu)?.length,
    2,
    "Unix and Windows smoke must install the local Protocol archive",
  );
  assert.equal(
    workflow.match(/publish-protocol\.mjs check/gu)?.length,
    2,
    "Unix and Windows CI must validate the exact Protocol archive without publishing",
  );
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

test("current source stays product-only while historical release evidence remains linked", async () => {
  const [packageText, readme] = await Promise.all([
    readFile(new URL("../../package.json", import.meta.url), "utf8"),
    readFile(new URL("../../README.md", import.meta.url), "utf8"),
  ]);
  const packageJson = JSON.parse(packageText) as {
    files?: string[];
    private?: boolean;
    version?: string;
  };
  assert.equal(packageJson.version, "0.8.0");
  assert.equal(
    packageJson.private,
    true,
    "post-release source must not republish immutable Canopus 0.8.0",
  );
  for (const file of [
    "README.md",
    "THIRD_PARTY.md",
    "docs/MISSIONS.md",
    "docs/RUN_RECORD.md",
    "docs/adr/0010-nonmutating-runs-and-explicit-submission.md",
  ]) {
    assert.ok(packageJson.files?.includes(file), `${file} must ship in the npm package`);
  }
  for (const historical of [
    "BUILD_WEEK.md",
    "docs/RELEASES.md",
    "advisories",
    "evidence/build-week",
    "evidence/erdos",
    "scripts/run-claim-fidelity-advisory.mjs",
    "evaluation",
    "toolchain.lock.json",
  ]) {
    assert.equal(
      packageJson.files?.includes(historical),
      false,
      `${historical} must remain source-only`,
    );
  }
  assert.equal(
    packageJson.files?.includes("dist/src/capability"),
    false,
    "the installed package must not ship the retired long-lived key store",
  );
  assert.match(
    readme,
    /immutable public product is Canopus `0\.8\.0`[\s\S]+toolchain\.lock\.json/u,
  );
  assert.match(readme, /A Run is nonmutating/u);
  assert.match(readme, /only canonical `vela submit` registers/u);
  assert.doesNotMatch(readme, /canopus submit\b/u);
  assert.doesNotMatch(readme, /canopus land|canopus inspect|canopus withdraw/u);
  assert.match(
    readme,
    /github\.com\/vela-science\/vela-research-harness\/blob\/v0\.6\.5\/BUILD_WEEK\.md/u,
  );
});
