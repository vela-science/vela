import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

test("published Canopus is one inert product with one authority-free protocol dependency", async () => {
  const manifest = JSON.parse(
    await readFile(new URL("../../package.json", import.meta.url), "utf8"),
  ) as {
    name?: string;
    bin?: Record<string, string>;
    engines?: Record<string, string>;
    dependencies?: Record<string, string>;
    optionalDependencies?: Record<string, string>;
    peerDependencies?: Record<string, string>;
    scripts?: Record<string, string>;
    files?: string[];
  };

  assert.equal(manifest.name, "@vela-science/canopus");
  assert.deepEqual(manifest.dependencies ?? {}, {
    "@vela-science/protocol": "workspace:*",
  });
  assert.deepEqual(manifest.optionalDependencies ?? {}, {});
  assert.deepEqual(manifest.peerDependencies ?? {}, {});
  for (const lifecycle of ["preinstall", "install", "postinstall", "prepare"] as const) {
    assert.equal(manifest.scripts?.[lifecycle], undefined, `${lifecycle} must not execute on install`);
  }
  assert.equal(manifest.bin?.canopus, "dist/src/cli.js");
  assert.equal(manifest.engines?.node, ">=22 <23 || >=24 <25");
  for (const capsule of [
    "capsules/erdos1056-k15/bin/linux-arm64/10429201-10429400/verifier",
    "capsules/erdos1056-k15/bin/linux-x86_64/10429201-10429400/verifier",
    "capsules/formal-erdos-505-test-dim-one/verifier",
  ]) {
    assert.equal(manifest.files?.includes(capsule), true, `${capsule} must ship in the tarball`);
  }
  for (const historical of [
    "BUILD_WEEK.md",
    "docs/RELEASES.md",
    "advisories",
    "benchmarks",
    "experiments",
    "registrations",
    "video",
    "evidence/build-week",
    "evidence/erdos",
    "scripts/run-claim-fidelity-advisory.mjs",
  ]) {
    assert.equal(
      manifest.files?.includes(historical),
      false,
      `${historical} belongs to Git history, not the installed product`,
    );
  }

  const compiled = await readdir(new URL("../src/", import.meta.url), { recursive: true });
  assert.equal(
    compiled.some((entry) => entry.endsWith(".map")),
    false,
    "published output must not contain maps whose TypeScript sources are absent",
  );
});
