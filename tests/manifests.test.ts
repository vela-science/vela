import assert from "node:assert/strict";
import test from "node:test";

import type { Mission } from "../src/contracts/mission.js";
import type { EngineResult } from "../src/engines/engine.js";
import { engineManifest, verifierManifest } from "../src/evidence/manifests.js";
import { contentDigest } from "../src/util/canonical.js";

const digest = `sha256:${"a".repeat(64)}`;

test("engine and verifier manifests coexist under their full content identities", () => {
  const engine = {
    draft: {
      schema: "canopus.engine-output.v0",
      status: "success",
      claim: "fixture",
      artifacts: [],
      observations: [],
      caveats: [],
    },
    engine: {
      name: "codex-tools-native",
      version: "codex-cli 0.139.0",
      binary_sha256: digest,
      model: "fixture",
      configuration_sha256: digest,
    },
    usage: {
      input_tokens: 1,
      cached_input_tokens: 0,
      output_tokens: 1,
      reasoning_output_tokens: 0,
    },
    wallTimeMs: 1,
    eventTypes: ["turn.completed"],
    actionTypes: [],
    eventsDigest: digest,
    stderrDigest: digest,
  } as EngineResult;
  const mission = {
    verifier: {
      argv: ["verify/capsule", "--one"],
      executable_sha256: digest,
      cwd: "verify",
    },
  } as Mission;
  const firstEngine = engineManifest(engine);
  const firstVerifier = verifierManifest(mission);
  assert.notEqual(firstEngine.path, firstVerifier.path);
  assert.equal(firstEngine.path.includes(contentDigest(firstEngine.value).slice(7)), true);
  assert.equal(firstVerifier.path.includes(contentDigest(firstVerifier.value).slice(7)), true);

  const changedMission = structuredClone(mission);
  changedMission.verifier.argv = ["verify/capsule", "--two"];
  assert.notEqual(verifierManifest(changedMission).path, firstVerifier.path);
});

test("Mission v1 reports the actual container boundary and bound platform", () => {
  const mission = {
    schema: "canopus.mission.v1",
    verifier: {
      argv: ["capsule/verifier", "{artifact:proof.lean}"],
      executable_sha256: digest,
      cwd: "site",
      image: digest,
      platform: "linux/amd64",
    },
  } as Mission;
  const manifest = verifierManifest(mission).value;
  assert.equal(manifest.schema, "canopus.verifier-manifest.v1");
  assert.equal(manifest.image, digest);
  assert.deepEqual(manifest.platform, { os: "linux", arch: "amd64" });
  assert.deepEqual(manifest.sandbox, {
    backend: "docker",
    network: "deny",
    root_filesystem: "read_only",
    bind_mounts: "exact_inputs_read_only",
    capabilities: "drop_all",
    privilege_escalation: "deny",
  });
});
