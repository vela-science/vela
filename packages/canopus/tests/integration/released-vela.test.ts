import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import type { Mission } from "../../src/contracts/mission.js";
import { FakeEngine } from "../helpers/fake-engine.js";
import { runCanopus } from "../../src/run.js";
import { isolatedEnvironment } from "../../src/util/command.js";
import { sha256Bytes } from "../../src/util/canonical.js";
import { VelaClient } from "../../src/vela/cli.js";

const exec = promisify(execFile);
const velaBinary = process.env.CANOPUS_VELA_BIN;
const registeredVelaDigest = process.env.CANOPUS_VELA_SHA256;
const registeredVelaVersion = process.env.CANOPUS_VELA_VERSION;
const enabled =
  velaBinary !== undefined &&
  registeredVelaDigest !== undefined &&
  registeredVelaVersion !== undefined;

async function command(
  binary: string,
  args: string[],
  cwd: string,
  home: string,
  extraEnvironment: NodeJS.ProcessEnv = {},
): Promise<string> {
  const result = await exec(binary, args, {
    cwd,
    encoding: "utf8",
    env: { ...isolatedEnvironment(home), ...extraEnvironment },
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

test(
  "released Vela offer, nonmutating Run, and clean-clone verifier compose",
  {
    skip: enabled
      ? false
      : "set CANOPUS_VELA_BIN, CANOPUS_VELA_SHA256, and CANOPUS_VELA_VERSION",
  },
  async (context) => {
    assert.ok(velaBinary !== undefined);
    assert.ok(registeredVelaDigest !== undefined);
    assert.ok(registeredVelaVersion !== undefined);
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
    const authorityKey = path.join(parent, "authority");
    await command(
      "/usr/bin/ssh-keygen",
      ["-q", "-t", "ed25519", "-N", "", "-C", "canopus released fixture", "-f", authorityKey],
      parent,
      setupHome,
    );
    const agentSocket = path.join(
      "/tmp",
      `canopus-released-agent-${process.pid}-${Date.now()}`,
    );
    const agentOutput = await command(
      "/usr/bin/ssh-agent",
      ["-a", agentSocket, "-s"],
      parent,
      setupHome,
    );
    const agentPid = agentOutput.match(/SSH_AGENT_PID=([0-9]+);/u)?.[1];
    assert.ok(agentPid !== undefined);
    const authorityEnvironment = {
      SSH_AUTH_SOCK: agentSocket,
      SSH_AGENT_PID: agentPid,
    };
    context.after(async () => {
      await command(
        "/usr/bin/ssh-agent",
        ["-k"],
        parent,
        setupHome,
        authorityEnvironment,
      ).catch(() => undefined);
    });
    await command(
      "/usr/bin/ssh-add",
      [authorityKey],
      parent,
      setupHome,
      authorityEnvironment,
    );
    await command(
      velaBinary,
      [
        "authority", "init", source,
        "--reason", "Establish disposable authority for the released Canopus fixture.",
        "--json",
      ],
      parent,
      setupHome,
      authorityEnvironment,
    );
    const domainDirectory = path.join(source, "domain");
    await mkdir(domainDirectory);
    await writeFile(path.join(domainDirectory, "source.json"), "{\"open\":[\"seed:canopus-smoke\"]}\n");
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
      ["-c", "core.hooksPath=/dev/null", "commit", "--no-gpg-sign", "-m", "Add target source"],
      source,
      setupHome,
    );
    const sourceCommit = await command(
      "git",
      ["rev-parse", "HEAD^{commit}"],
      source,
      setupHome,
    );
    const status = JSON.parse(await command(
      velaBinary,
      ["status", source, "--json"],
      parent,
      setupHome,
    )) as { frontier: { id: string } };
    const packetDirectory = path.join(source, "site", "problems");
    await mkdir(packetDirectory, { recursive: true });
    const packetPath = path.join(packetDirectory, "canopus-smoke.json");
    await writeFile(
      packetPath,
      "{\"schema\":\"canopus.fixture-work.v1\",\"target\":\"seed:canopus-smoke\"}\n",
    );
    const candidateDirectory = path.join(source, ".vela", "tmp");
    await mkdir(candidateDirectory, { recursive: true });
    const candidate = path.join(candidateDirectory, "target-index-candidate.json");
    await writeFile(candidate, JSON.stringify({
      schema: "vela.target-index-candidate.v1",
      frontier_id: status.frontier.id,
      source: {
        git_commit: sourceCommit,
        input_paths: ["domain/source.json"],
      },
      targets: [{
        id: "seed:canopus-smoke",
        title: "Verify one exact bounded JSON artifact",
        why: "Exercise the released read-only offer and Canopus verifier path.",
        state: "open",
        rank: 1,
        objective: "Produce one exact bounded JSON witness.",
        labels: ["canopus", "fixture"],
        packet: {
          schema: "canopus.fixture-work.v1",
          path: "site/problems/canopus-smoke.json",
        },
      }],
    }, null, 2));
    await command(
      velaBinary,
      [
        "target-index", "seal", source,
        "--candidate", candidate,
        "--apply",
        "--json",
      ],
      parent,
      setupHome,
    );
    await command("git", ["add", "-A"], source, setupHome);
    await command(
      "git",
      ["-c", "core.hooksPath=/dev/null", "commit", "--no-gpg-sign", "-m", "Seal Canopus target index"],
      source,
      setupHome,
    );

    const vela = new VelaClient({
      binary: velaBinary,
      expectedVersion: registeredVelaVersion,
      expectedSha256: registeredVelaDigest,
      home: path.join(runRoot, "vela-home"),
    });
    const initial = await vela.inspect(source, ".");
    const mission: Mission = {
      schema: "canopus.mission.v0",
      id: "mission_released_vela_smoke",
      target: "seed:canopus-smoke",
      vela_version: registeredVelaVersion,
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
    });
    assert.equal(result.record.schema, "canopus.run.v2");
    assert.equal(result.record.effect, "none");
    assert.equal(result.record.submission, null);
    assert.equal(result.record.reproduction.matched, true);
    assert.equal(result.record.external_gate_credit, false);
    const activity = await readFile(path.join(result.paths.root, "activity.jsonl"), "utf8");
    const activityEvents = activity
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line) as {
        type: string;
        payload: Record<string, unknown>;
      });
    assert.equal(activityEvents.some((event) => event.type === "work.claimed"), false);
    assert.equal(activityEvents.some((event) => event.type === "work.skipped"), true);
    assert.equal(activityEvents.some((event) => event.type === "artifacts.published"), false);
    assert.equal(
      await command("git", ["rev-parse", "HEAD^{commit}"], source, setupHome),
      initial.roots.git_commit,
    );
    await assert.rejects(lstat(result.paths.velaHome), /ENOENT/u);
    await assert.rejects(lstat(path.join(parent, "capabilities")), /ENOENT/u);
    const sourcePrivateEntries = await readdir(path.join(source, ".vela"), {
      recursive: true,
    });
    assert.equal(sourcePrivateEntries.some((entry) => /private\.key|human/i.test(entry)), false);
  },
);
