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

  assert.equal(SUPPORTED_VELA_VERSION, "0.915.1");
  assert.equal(SUPPORTED_CODEX_VERSION, "0.145.0");
  assert.match(workflow, /releases\/download\/v0\.915\.1/u);
  assert.match(workflow, /codex-0\.145\.0-linux-x64\.tgz/u);
  assert.match(
    workflow,
    /actions\/checkout@[^\n]+\n\s+with:\n\s+fetch-depth: 0/u,
    "historical registration checks require full Git history",
  );
  assert.doesNotMatch(workflow, /releases\/download\/v0\.914\.1/u);
  assert.doesNotMatch(workflow, /codex-0\.144\.6-linux-x64\.tgz/u);
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
  assert.match(
    workflow,
    /actions\/checkout@[^\n]+\n\s+with:\n\s+fetch-depth: 0/u,
    "release validation must retain history for immutable registration replay",
  );
});

test("published package carries the exact Build Week judge path", async () => {
  const [packageText, readme, buildWeek] = await Promise.all([
    readFile(new URL("../../package.json", import.meta.url), "utf8"),
    readFile(new URL("../../README.md", import.meta.url), "utf8"),
    readFile(new URL("../../BUILD_WEEK.md", import.meta.url), "utf8"),
  ]);
  const packageJson = JSON.parse(packageText) as { files?: string[]; version?: string };
  const artifact = "artifacts/sidon-a24-gpt56-7194.witness.json";
  const auditCommit = "825657d7e87618c0aa6fc9af7e3182e05f324750";
  const velaRelease = "https://github.com/vela-science/vela/releases/tag/v0.912.0";

  assert.equal(packageJson.version, "0.6.4");
  for (const file of [
    "README.md",
    "BUILD_WEEK.md",
    "THIRD_PARTY.md",
    "docs/RELEASES.md",
    "evidence/erdos",
  ]) {
    assert.ok(packageJson.files?.includes(file), `${file} must ship in the npm package`);
  }
  for (const document of [readme, buildWeek]) {
    assert.match(document, new RegExp(velaRelease.replaceAll(".", "\\."), "u"));
    assert.match(document, new RegExp(`git checkout ${auditCommit}`, "u"));
    assert.match(document, new RegExp(`vela reproduce ${artifact.replaceAll(".", "\\.")}`, "u"));
    assert.match(document, /node verification\/verify-sidon-a24-7194\.mjs/u);
  }
});
