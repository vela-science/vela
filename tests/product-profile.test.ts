import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, symlink } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  loadProductProfile,
  loadProfileDraft,
  stageProfileCapsule,
  verifierImageAt,
} from "../src/product/profile.js";
import {
  listProductProfiles,
  packProductProfile,
  validateProductProfile,
} from "../src/product/profile-bundle.js";
import { resolveProductProfile, selectProductOffer } from "../src/product/doctor.js";
import { contentDigest } from "../src/util/canonical.js";
import { assertVerifierWorkingDirectory } from "../src/mission/prepare.js";

test("verifier cwd must exist below the sealed source before a model call", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-verifier-cwd-"));
  await mkdir(path.join(root, "targets"));
  await assertVerifierWorkingDirectory(root, "targets");
  await assert.rejects(
    assertVerifierWorkingDirectory(root, "site"),
    /does not exist in the sealed source checkout/u,
  );
  await symlink(os.tmpdir(), path.join(root, "escape"));
  await assert.rejects(
    assertVerifierWorkingDirectory(root, "escape"),
    /not a real directory below the sealed source checkout/u,
  );
});

test("registered product profiles stage exact distinct capsules and bounded Mission v1 drafts", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-product-profiles-"));
  const profiles = [
    await loadProductProfile("erdos1056-k15-10429001-10429200", { platform: "darwin-arm64" }),
    await loadProductProfile("erdos1056-k15-10429001-10429200", { platform: "linux-x86_64" }),
    await loadProductProfile("formal-erdos-505-test-dim-one-gpt56", { platform: "darwin-arm64" }),
    await loadProductProfile("formal-erdos-505-test-dim-one-gpt56", { platform: "linux-x86_64" }),
    await loadProductProfile("quantum-10-1-4-stabilizer-retry", { platform: "darwin-arm64" }),
    await loadProductProfile("quantum-10-1-4-stabilizer-retry", { platform: "linux-x86_64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56", { platform: "darwin-arm64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56", { platform: "linux-x86_64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56-v2", { platform: "darwin-arm64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56-v2", { platform: "linux-x86_64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56-v3", { platform: "darwin-arm64" }),
    await loadProductProfile("sidon-a24-at-least-7194-gpt56-v3", { platform: "linux-x86_64" }),
  ];
  assert.equal(profiles[0]?.target, "erdos:1056");
  assert.notEqual(profiles[0]?.capsule_sha256, profiles[1]?.capsule_sha256);
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[0]!)),
    contentDigest(await loadProfileDraft(profiles[1]!)),
  );
  assert.equal(profiles[2]?.target, "formal:erdos-505-test-dim-one");
  assert.equal(profiles[2]?.capsule_sha256, profiles[3]?.capsule_sha256);
  assert.equal(profiles[2]?.verifier_platform, "linux/amd64");
  assert.equal(profiles[3]?.verifier_platform, "linux/amd64");
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[2]!)),
    contentDigest(await loadProfileDraft(profiles[3]!)),
  );
  const formalDraft = await loadProfileDraft(profiles[2]!) as {
    verifier: { cwd: string };
  };
  assert.equal(formalDraft.verifier.cwd, "targets");
  assert.equal(profiles[4]?.target, "quantum:[[10,1,4]]");
  assert.equal(profiles[4]?.capsule_sha256, profiles[5]?.capsule_sha256);
  assert.equal(profiles[4]?.verifier_platform, "linux/arm64");
  assert.equal(profiles[5]?.verifier_platform, "linux/amd64");
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[4]!)),
    contentDigest(await loadProfileDraft(profiles[5]!)),
  );
  assert.equal(profiles[6]?.target, "sidon:a24-improve");
  assert.notEqual(profiles[6]?.capsule_sha256, profiles[7]?.capsule_sha256);
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[6]!)),
    contentDigest(await loadProfileDraft(profiles[7]!)),
  );
  assert.equal(profiles[8]?.target, "sidon:a24-improve");
  assert.notEqual(profiles[8]?.capsule_sha256, profiles[9]?.capsule_sha256);
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[8]!)),
    contentDigest(await loadProfileDraft(profiles[9]!)),
  );
  assert.equal(profiles[10]?.target, "sidon:a24-improve");
  assert.notEqual(profiles[10]?.capsule_sha256, profiles[11]?.capsule_sha256);
  assert.equal(
    contentDigest(await loadProfileDraft(profiles[10]!)),
    contentDigest(await loadProfileDraft(profiles[11]!)),
  );
  for (const [index, profile] of profiles.entries()) {
    const staging = path.join(root, `${profile.name}-${index}`);
    await mkdir(staging);
    const staged = await stageProfileCapsule({ profile, stagingRoot: staging });
    assert.equal(staged.source, "packaged");
  }
});

test("portable verifier images require a closed public repository and full digest", () => {
  assert.equal(
    verifierImageAt(
      `ghcr.io/vela-science/canopus-verifier@sha256:${"a".repeat(64)}`,
    ),
    `ghcr.io/vela-science/canopus-verifier@sha256:${"a".repeat(64)}`,
  );
  assert.equal(
    verifierImageAt(
      `ghcr.io/vela-science/canopus-formal-verifier@sha256:${"b".repeat(64)}`,
    ),
    `ghcr.io/vela-science/canopus-formal-verifier@sha256:${"b".repeat(64)}`,
  );
  assert.throws(() => verifierImageAt(`sha256:${"a".repeat(64)}`), /length|invalid format/u);
  assert.throws(
    () => verifierImageAt(`ghcr.io/other/canopus-verifier@sha256:${"a".repeat(64)}`),
    /length|invalid format/u,
  );
  assert.throws(
    () => verifierImageAt("ghcr.io/vela-science/canopus-verifier:latest"),
    /length|invalid format/u,
  );
  assert.throws(
    () =>
      verifierImageAt(
        `ghcr.io/vela-science/canopus-unregistered-verifier@sha256:${"a".repeat(64)}`,
      ),
    /invalid format/u,
  );
});

test("profile v2 binds exact platform custody and packs only portable contract resources", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-profile-pack-parent-"));
  const name = "erdos1056-k15-10429001-10429200";
  assert.deepEqual(await listProductProfiles(), [
    name,
    "formal-erdos-505-test-dim-one-gpt56",
    "quantum-10-1-4-stabilizer-retry",
    "sidon-a24-at-least-7194-gpt56",
    "sidon-a24-at-least-7194-gpt56-v2",
    "sidon-a24-at-least-7194-gpt56-v3",
  ]);
  const mac = await loadProductProfile(name, { platform: "darwin-arm64" });
  const linux = await loadProductProfile(name, { platform: "linux-x86_64" });
  assert.equal(mac.target_packet_schema, "erdos-frontier.problem-work.v1");
  assert.equal(mac.permission_profile, "runtime/native-worker/config.toml");
  assert.equal(linux.permission_profile, "runtime/native-worker/config-linux.toml");
  assert.notEqual(mac.capsule_sha256, linux.capsule_sha256);
  assert.equal(mac.landing.max_accepted_delta, 0);
  assert.deepEqual(mac.landing.expected_routes, ["defer"]);

  const validation = await validateProductProfile(name);
  assert.equal(validation.schema, "canopus.profile-validation.v1");
  assert.equal(validation.platforms["darwin-arm64"].verifier_capsule_sha256, mac.capsule_sha256);
  assert.equal(validation.platforms["linux-x86_64"].verifier_capsule_sha256, linux.capsule_sha256);

  const output = path.join(root, "bundle");
  const packed = await packProductProfile(name, output);
  const manifest = JSON.parse(await readFile(packed.manifest, "utf8")) as {
    schema: string;
    files: Array<{ path: string }>;
  };
  assert.equal(manifest.schema, "canopus.profile-pack.v1");
  assert.equal(packed.files, 6);
  assert.deepEqual(manifest.files.map((file) => file.path), [
    "capsules/erdos1056-k15/bin/linux-arm64/10429001-10429200/verifier",
    "capsules/erdos1056-k15/bin/linux-x86_64/10429001-10429200/verifier",
    "missions/erdos1056-k15-next/mission.draft.json",
    `profiles/${name}.json`,
    "runtime/native-worker/config-linux.toml",
    "runtime/native-worker/config.toml",
  ]);
});

test("formal profile packs one shared capsule with exact amd64 emulation", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-formal-profile-pack-parent-"));
  const name = "formal-erdos-505-test-dim-one-gpt56";
  const validation = await validateProductProfile(name);
  assert.equal(validation.platforms["darwin-arm64"].verifier_platform, "linux/amd64");
  assert.equal(validation.platforms["linux-x86_64"].verifier_platform, "linux/amd64");

  const packed = await packProductProfile(name, path.join(root, "bundle"));
  const manifest = JSON.parse(await readFile(packed.manifest, "utf8")) as {
    files: Array<{ path: string }>;
  };
  assert.equal(packed.files, 5);
  assert.deepEqual(manifest.files.map((file) => file.path), [
    "capsules/formal-erdos-505-test-dim-one/verifier",
    "missions/formal-erdos-505-test-dim-one-gpt56/mission.draft.json",
    `profiles/${name}.json`,
    "runtime/native-worker/config-linux.toml",
    "runtime/native-worker/config.toml",
  ]);
});

test("quantum profile packs the frozen witness verifier and bounded retry", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-quantum-profile-pack-parent-"));
  const name = "quantum-10-1-4-stabilizer-retry";
  const validation = await validateProductProfile(name);
  assert.equal(validation.platforms["darwin-arm64"].verifier_platform, "linux/arm64");
  assert.equal(validation.platforms["linux-x86_64"].verifier_platform, "linux/amd64");

  const packed = await packProductProfile(name, path.join(root, "bundle"));
  const manifest = JSON.parse(await readFile(packed.manifest, "utf8")) as {
    files: Array<{ path: string }>;
  };
  assert.equal(packed.files, 5);
  assert.deepEqual(manifest.files.map((file) => file.path), [
    "capsules/quantum-10-1-4/verifier.py",
    "missions/quantum-10-1-4-retry/mission.draft.json",
    `profiles/${name}.json`,
    "runtime/native-worker/config-linux.toml",
    "runtime/native-worker/config.toml",
  ]);
});

test("Linux custody denies host roots and reopens only the exact workspace", async () => {
  const config = await readFile(
    path.resolve("runtime/native-worker/config-linux.toml"),
    "utf8",
  );
  assert.match(config, /^"\/home" = "deny"$/mu);
  assert.match(config, /^"\/root" = "deny"$/mu);
  assert.match(config, /^"\/tmp" = "deny"$/mu);
  assert.match(
    config,
    /\[permissions\.canopus-worker\.filesystem\.":workspace_roots"\]\n"\." = "write"\n"\.canopus-runtime" = "read"/u,
  );
  assert.doesNotMatch(config, /^"\/" = "write"$/mu);
});

test("explicit targets are deliberate while the default never skips rank one", async () => {
  const profile = await loadProductProfile("erdos1056-k15-10429001-10429200");
  const offer = {
    targets: [
      { rank: 1, target_id: "erdos:124" },
      { rank: 2, target_id: "erdos:1056" },
    ],
  };
  assert.throws(() => selectProductOffer(offer, profile), /will not skip rank 1/u);
  assert.deepEqual(selectProductOffer(offer, profile, "erdos:1056"), {
    target: { rank: 2, target_id: "erdos:1056" },
    targetId: "erdos:1056",
    rank: 2,
  });
  assert.throws(
    () => selectProductOffer(offer, profile, "erdos:124"),
    /not requested target erdos:124/u,
  );
});

test("ordinary profile discovery selects the unique first-offer profile", async () => {
  const profile = await resolveProductProfile({
    targets: [{ rank: 1, target_id: "erdos:1056" }],
  });
  assert.equal(profile.name, "erdos1056-k15-10429001-10429200");
  const formal = await resolveProductProfile({
    targets: [{ rank: 1, target_id: "formal:erdos-505-test-dim-one" }],
  });
  assert.equal(formal.name, "formal-erdos-505-test-dim-one-gpt56");
  const quantum = await resolveProductProfile({
    targets: [{ rank: 1, target_id: "quantum:[[10,1,4]]" }],
  });
  assert.equal(quantum.name, "quantum-10-1-4-stabilizer-retry");
  await assert.rejects(
    resolveProductProfile({
      availability: { configured_open: 1, available: 0, leased: 1 },
      leased_targets: [{
        target_id: "sidon:a24-improve",
        actor: "agent:canopus-local",
        expires_at: "2026-07-21T22:03:46Z",
      }],
      targets: [],
    }),
    /sidon:a24-improve by agent:canopus-local until 2026-07-21T22:03:46Z/u,
  );
  await assert.rejects(
    resolveProductProfile({ targets: [{ rank: 1, target_id: "unknown:target" }] }),
    /no runnable profile is registered/u,
  );
});
