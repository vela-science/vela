import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import {
  retainWithdrawalCapability,
  withdrawalCapabilityStatus,
} from "../../src/capability/withdrawal.js";
import type { Mission } from "../../src/contracts/mission.js";
import { FakeEngine } from "../../src/engines/fake.js";
import { withdrawProduct } from "../../src/product/withdraw.js";
import { runCanopus } from "../../src/run.js";
import { isolatedEnvironment } from "../../src/util/command.js";
import { sha256Bytes } from "../../src/util/canonical.js";
import { retainedArtifactPath, VelaClient } from "../../src/vela/cli.js";

const exec = promisify(execFile);
const velaBinary = process.env.CANOPUS_VELA_BIN;
const registeredVelaDigest = process.env.CANOPUS_VELA_SHA256;
const enabled = velaBinary !== undefined && registeredVelaDigest !== undefined;

async function command(
  binary: string,
  args: string[],
  cwd: string,
  home: string,
): Promise<string> {
  const result = await exec(binary, args, {
    cwd,
    encoding: "utf8",
    env: isolatedEnvironment(home),
    maxBuffer: 8 * 1024 * 1024,
    timeout: 30_000,
  });
  return result.stdout.trim();
}

async function removeSealedTree(root: string): Promise<void> {
  try {
    const metadata = await lstat(root);
    if (metadata.isDirectory()) {
      await chmod(root, 0o700);
      for (const child of await readdir(root)) await removeSealedTree(path.join(root, child));
    } else if (!metadata.isSymbolicLink()) {
      await chmod(root, 0o600);
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  await rm(root, { recursive: true, force: true });
}

async function assertTreeOmitsBytes(root: string, forbidden: Buffer, base = root): Promise<void> {
  const metadata = await lstat(root);
  if (metadata.isDirectory()) {
    for (const child of await readdir(root)) {
      await assertTreeOmitsBytes(path.join(root, child), forbidden, base);
    }
    return;
  }
  if (metadata.isFile()) {
    assert.equal(
      (await readFile(root)).includes(forbidden),
      false,
      `run evidence leaked proposal-scoped secret bytes at ${path.relative(base, root)}`,
    );
  }
}

test(
  "released Vela work, Defer landing, record binding, and clean-clone verifier compose",
  { skip: enabled ? false : "set CANOPUS_VELA_BIN and CANOPUS_VELA_SHA256" },
  async (context) => {
    assert.ok(velaBinary !== undefined);
    assert.ok(registeredVelaDigest !== undefined);
    const parent = await mkdtemp(path.join(os.tmpdir(), "canopus-released-vela-"));
    context.after(async () => await removeSealedTree(parent));
    const source = path.join(parent, "source");
    const setupHome = path.join(parent, "setup-home");
    const runRoot = path.join(parent, "run");
    await mkdir(setupHome);
    const observedVelaDigest = sha256Bytes(await readFile(velaBinary));
    assert.equal(observedVelaDigest, registeredVelaDigest);
    await command(
      velaBinary,
      [
        "init", source,
        "--name", "canopus-released-smoke",
        "--scope", "Verify one exact bounded JSON artifact.",
        "--json",
      ],
      parent,
      setupHome,
    );
    const verifierDirectory = path.join(source, "verifier");
    await mkdir(verifierDirectory);
    const verifierSource = path.join(parent, "check-json.c");
    const verifier = path.join(verifierDirectory, "check-json");
    await writeFile(
      verifierSource,
      `#include <stdio.h>\n#include <string.h>\nint main(int argc, char **argv) {\n  if (argc != 2) return 2;\n  char bytes[64] = {0}; FILE *file = fopen(argv[1], "r");\n  if (!file) return 3; size_t count = fread(bytes, 1, sizeof(bytes), file); fclose(file);\n  const char *expected = "{\\\"value\\\":42}\\n";\n  return count == strlen(expected) && memcmp(bytes, expected, count) == 0 ? 0 : 4;\n}\n`,
    );
    await exec("/usr/bin/clang", ["-Os", "-o", verifier, verifierSource]);
    await chmod(verifier, 0o555);
    await writeFile(
      path.join(source, "campaign.yaml"),
      `batches:\n  - name: Canopus released-interface smoke\n    state: open\n    problems:\n      - id: seed:canopus-smoke\n        title: Verify one exact bounded JSON artifact\n        why: Exercise work, authored Defer landing, and clean-clone replay\n`,
    );
    await command("git", ["config", "user.name", "Canopus Integration"], source, setupHome);
    await command(
      "git",
      ["config", "user.email", "canopus@example.invalid"],
      source,
      setupHome,
    );
    await command("git", ["add", "-A"], source, setupHome);
    await command(
      "git",
      ["-c", "core.hooksPath=/dev/null", "commit", "--no-gpg-sign", "-m", "Initialize Canopus Vela smoke frontier"],
      source,
      setupHome,
    );

    const vela = new VelaClient({
      binary: velaBinary,
      expectedVersion: "0.915.1",
      expectedSha256: registeredVelaDigest,
      home: path.join(runRoot, "vela-home"),
    });
    const initial = await vela.inspect(source, ".");
    const mission: Mission = {
      schema: "canopus.mission.v0",
      id: "mission_released_vela_smoke",
      target: "seed:canopus-smoke",
      vela_version: "0.915.1",
      vela_sha256: registeredVelaDigest,
      frontier: ".",
      actor: "agent:canopus-smoke",
      role: "producer",
      claim_type: "computational",
      replayability: "exact",
      objective: "Produce one exact bounded JSON witness.",
      completion_condition: "The committed verifier accepts the frozen bytes.",
      roots: initial.roots,
      allowed_paths: ["result.json"],
      budgets: {
        max_research_wall_time_ms: 30_000,
        max_research_processes: 3,
        max_research_output_bytes: 1_048_576,
        max_prompt_bytes: 16_384,
        max_artifact_bytes: 1_048_576,
        max_attempts: 1,
        max_observed_tokens: 1,
      },
      verifier: {
        argv: ["verifier/check-json", "{artifact:result.json}"],
        executable_sha256: sha256Bytes(await readFile(verifier)),
        cwd: "verifier",
        timeout_ms: 2000,
        max_output_bytes: 4096,
        network: "deny",
        writes: "deny",
      },
      scientific_chain: {
        predicted_observable: "The frozen result is the exact JSON object with value 42.",
        performed_test: "Ran verifier/check-json against the content-addressed bytes.",
      },
      landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
    };
    const capabilityStore = path.join(parent, "capabilities");
    const result = await runCanopus({
      mission,
      sourceRepo: source,
      runRoot,
      vela,
      engine: new FakeEngine({
        schema: "canopus.engine-output.v0",
        status: "success",
        claim: "The exact bounded result has value 42.",
        artifacts: [
          {
            path: "result.json",
            kind: "witness",
            encoding: "utf8",
            content: "{\"value\":42}\n",
          },
        ],
        observations: ["The deterministic fixture emitted one bounded result."],
        caveats: ["This smoke establishes interface composition, not a scientific claim."],
      }),
      retainWithdrawalCapability: async (context) => {
        await retainWithdrawalCapability({
          ...context,
          velaBinary,
          storeRoot: capabilityStore,
        });
      },
    });
    assert.equal(result.record.landing.route, "defer");
    assert.equal(result.record.landing.accepted_event_delta, 0);
    assert.equal(result.record.landing.publication_state, "committed_local");
    assert.equal(result.record.reproduction.matched, true);
    assert.equal(result.record.external_gate_credit, false);
    const witness = result.record.candidate.artifacts.find(
      (artifact) => artifact.path === "result.json",
    );
    assert.ok(witness !== undefined);
    assert.equal(
      await readFile(
        retainedArtifactPath(result.paths.landing, mission.frontier, witness.digest),
        "utf8",
      ),
      "{\"value\":42}\n",
    );
    await assert.rejects(lstat(result.paths.velaHome), /ENOENT/u);
    assert.equal(
      (await withdrawalCapabilityStatus(result.record.landing.proposal_id, capabilityStore)).available,
      true,
    );
    const retainedSecret = await readFile(
      path.join(capabilityStore, result.record.landing.proposal_id, "private.key"),
    );
    assert.equal(path.resolve(capabilityStore).startsWith(`${path.resolve(result.paths.root)}${path.sep}`), false);
    await assertTreeOmitsBytes(result.paths.root, retainedSecret);
    retainedSecret.fill(0);
    const withdrawal = await withdrawProduct({
      frontier: result.paths.landing,
      runFile: path.join(result.paths.root, "run.json"),
      reason: "released integration fixture completed",
      storeRoot: capabilityStore,
    });
    assert.equal(withdrawal.accepted_state_changed, false);
    assert.equal(withdrawal.clean_clone_reproduced, true);
    assert.equal(withdrawal.capability_consumed, true);
    assert.equal(
      withdrawal.scientific_state_root_before,
      withdrawal.scientific_state_root_after,
    );
    assert.equal(
      (await withdrawalCapabilityStatus(result.record.landing.proposal_id, capabilityStore)).available,
      false,
    );
    const sourcePrivateEntries = await readdir(path.join(source, ".vela"), {
      recursive: true,
    });
    assert.equal(sourcePrivateEntries.some((entry) => /private\.key|human/i.test(entry)), false);
  },
);
