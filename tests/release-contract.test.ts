import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  SUPPORTED_CODEX_VERSION,
  SUPPORTED_VELA_VERSION,
} from "../src/product/version.js";

test("current product release pins the tested Vela and Codex boundaries", async () => {
  const workflow = await readFile(
    new URL("../../.github/workflows/ci.yml", import.meta.url),
    "utf8",
  );

  assert.equal(SUPPORTED_VELA_VERSION, "0.940.7");
  assert.equal(SUPPORTED_CODEX_VERSION, "0.145.0");
  assert.match(workflow, /releases\/download\/v0\.940\.7/u);
  assert.match(workflow, /CANOPUS_VELA_VERSION="0\.940\.7"/u);
  assert.match(workflow, /codex-0\.145\.0-linux-x64\.tgz/u);
  assert.doesNotMatch(workflow, /releases\/download\/v0\.915\.1/u);
  assert.doesNotMatch(workflow, /codex-0\.144\.6-linux-x64\.tgz/u);
});

test("installed-package smoke validates the current packaged Erdős profile", async () => {
  const workflow = await readFile(
    new URL("../../.github/workflows/ci.yml", import.meta.url),
    "utf8",
  );
  const currentProfile = "erdos1056-k15-10429201-10429400";
  const supersededProfile = "erdos1056-k15-10428801-10429000";

  assert.equal(
    workflow.match(new RegExp(`profile validate ${currentProfile}`, "gu"))?.length,
    2,
    "Unix and Windows installed-package smoke must validate the current profile",
  );
  assert.doesNotMatch(
    workflow,
    new RegExp(`profile validate ${supersededProfile}`, "u"),
    "CI must not validate a profile omitted from the current package",
  );
});

test("release binds tag, GitHub attestation, and npm trusted provenance", async () => {
  const workflow = await readFile(
    new URL("../../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );

  for (const contract of [
    "environment: npm",
    "id-token: write",
    "test \"v$(node -p 'require(\"./package.json\").version')\" = \"$GITHUB_REF_NAME\"",
    "actions/attest-build-provenance@",
    "gh attestation verify",
    "--signer-workflow",
    "--source-ref",
    "--source-digest",
    "--deny-self-hosted-runners",
    "(cd release && shasum -a 256 *.tgz > SHA256SUMS)",
    "npm publish ./release/*.tgz --provenance --access public",
    "npm audit signatures --json --include-attestations",
    "https://slsa.dev/provenance/v1",
  ]) {
    assert.match(workflow, new RegExp(contract.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&"), "u"));
  }
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/u);
  assert.doesNotMatch(workflow, /shasum -a 256 release\/\*\.tgz/u);
});

test("current source stays product-only while historical release evidence remains linked", async () => {
  const [packageText, readme] = await Promise.all([
    readFile(new URL("../../package.json", import.meta.url), "utf8"),
    readFile(new URL("../../README.md", import.meta.url), "utf8"),
  ]);
  const packageJson = JSON.parse(packageText) as { files?: string[]; version?: string };
  assert.equal(packageJson.version, "0.8.0-rc.7");
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
  ]) {
    assert.equal(
      packageJson.files?.includes(historical),
      false,
      `${historical} must remain source evidence only`,
    );
  }
  assert.equal(
    packageJson.files?.includes("dist/src/capability"),
    false,
    "the installed package must not ship the retired long-lived key store",
  );
  assert.match(
    readme,
    /Current source is Canopus `0\.8\.0-rc\.7`, composed with Vela[\s\S]+`0\.940\.7`/u,
  );
  assert.match(readme, /A Run is nonmutating/u);
  assert.match(readme, /only the separate `submit` command registers/u);
  assert.doesNotMatch(readme, /canopus land|canopus inspect|canopus withdraw/u);
  assert.match(
    readme,
    /github\.com\/vela-science\/vela-research-harness\/blob\/v0\.6\.5\/BUILD_WEEK\.md/u,
  );
});
