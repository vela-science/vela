import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, symlink } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

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

test("the active product profile stages exact platform capsules and one bounded Mission v1 draft", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-product-profiles-"));
  const profiles = [
    await loadProductProfile("erdos1056-k15-10429601-10429800", { platform: "darwin-arm64" }),
    await loadProductProfile("erdos1056-k15-10429601-10429800", { platform: "linux-x86_64" }),
  ];
  assert.equal(profiles[0]?.target, "erdos:1056");
  assert.notEqual(profiles[0]?.capsule_sha256, profiles[1]?.capsule_sha256);
  const draft = await loadProfileDraft(profiles[0]!) as {
    verifier: { cwd: string };
  };
  assert.equal(draft.verifier.cwd, "targets");
  assert.equal(
    contentDigest(draft),
    contentDigest(await loadProfileDraft(profiles[1]!)),
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
  const name = "erdos1056-k15-10429601-10429800";
  assert.deepEqual(await listProductProfiles(), [
    name,
    "formal-erdos-505-test-dim-one",
  ]);
  const mac = await loadProductProfile(name, { platform: "darwin-arm64" });
  const linux = await loadProductProfile(name, { platform: "linux-x86_64" });
  assert.equal(mac.target_packet_schema, "erdos-frontier.problem-work.v2");
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
    "capsules/erdos1056-k15/bin/linux-arm64/10429601-10429800/verifier",
    "capsules/erdos1056-k15/bin/linux-x86_64/10429601-10429800/verifier",
    "missions/erdos1056-k15-next/mission.draft.json",
    `profiles/${name}.json`,
    "runtime/native-worker/config-linux.toml",
    "runtime/native-worker/config.toml",
  ]);
});

test("formal profile reuses the audited capsule and binds one exact Lean environment", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-formal-profile-pack-parent-"));
  const name = "formal-erdos-505-test-dim-one";
  const mac = await loadProductProfile(name, { platform: "darwin-arm64" });
  const linux = await loadProductProfile(name, { platform: "linux-x86_64" });
  assert.equal(mac.target, "formal:erdos-505-test-dim-one");
  assert.equal(mac.target_packet_schema, "formal-conjectures.lean-proof-work.v1");
  assert.equal(mac.capsule_sha256, linux.capsule_sha256);
  assert.equal(mac.verifier_platform, "linux/amd64");
  assert.equal(linux.verifier_platform, "linux/amd64");
  assert.equal(mac.landing.max_accepted_delta, 0);
  assert.deepEqual(mac.landing.expected_routes, ["defer"]);
  const draft = await loadProfileDraft(mac) as {
    budgets: { max_observed_tokens: number };
    verifier: { cwd: string };
    worker: { model: string };
  };
  assert.equal(draft.budgets.max_observed_tokens, 300_000);
  assert.equal(draft.verifier.cwd, "targets");
  assert.equal(draft.worker.model, "gpt-5.4");

  const validation = await validateProductProfile(name);
  assert.equal(validation.schema, "canopus.profile-validation.v1");
  assert.equal(
    validation.platforms["darwin-arm64"].verifier_capsule_sha256,
    mac.capsule_sha256,
  );
  assert.equal(
    validation.platforms["linux-x86_64"].verifier_capsule_sha256,
    linux.capsule_sha256,
  );

  const output = path.join(root, "bundle");
  const packed = await packProductProfile(name, output);
  const manifest = JSON.parse(await readFile(packed.manifest, "utf8")) as {
    schema: string;
    files: Array<{ path: string }>;
  };
  assert.equal(manifest.schema, "canopus.profile-pack.v1");
  assert.equal(packed.files, 5);
  assert.deepEqual(manifest.files.map((file) => file.path), [
    "capsules/formal-erdos-505-test-dim-one/verifier",
    "missions/formal-erdos-505-test-dim-one/mission.draft.json",
    `profiles/${name}.json`,
    "runtime/native-worker/config-linux.toml",
    "runtime/native-worker/config.toml",
  ]);
});

test("Linux custody denies host roots and reopens only the exact workspace", async () => {
  const config = await readFile(
    fileURLToPath(
      new URL("../../runtime/native-worker/config-linux.toml", import.meta.url),
    ),
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
  const profile = await loadProductProfile("erdos1056-k15-10429601-10429800");
  const packet = {
    schema: "erdos-frontier.problem-work.v2",
    sha256: "sha256:8d879e24a537de3b9b13ad7878dc98db8ce4f5273187c7f45d0d49a93e8fe8ad",
  };
  const offer = {
    targets: [
      { rank: 1, target_id: "erdos:124" },
      { rank: 2, target_id: "erdos:1056", packet },
    ],
  };
  assert.throws(() => selectProductOffer(offer, profile), /will not skip rank 1/u);
  assert.deepEqual(selectProductOffer(offer, profile, "erdos:1056"), {
    target: { rank: 2, target_id: "erdos:1056", packet },
    targetId: "erdos:1056",
    rank: 2,
  });
  assert.throws(
    () => selectProductOffer(offer, profile, "erdos:124"),
    /not requested target erdos:124/u,
  );
  assert.throws(
    () => selectProductOffer({
      targets: [{
        rank: 1,
        target_id: "erdos:1056",
        packet: { ...packet, sha256: `sha256:${"0".repeat(64)}` },
      }],
    }, profile),
    /registered profile.+is stale/u,
  );
});

test("ordinary profile discovery selects the unique first-offer profile", async () => {
  const profile = await resolveProductProfile({
    targets: [{
      rank: 1,
      target_id: "erdos:1056",
      packet: {
        schema: "erdos-frontier.problem-work.v2",
        sha256: "sha256:8d879e24a537de3b9b13ad7878dc98db8ce4f5273187c7f45d0d49a93e8fe8ad",
      },
    }],
  });
  assert.equal(profile.name, "erdos1056-k15-10429601-10429800");
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
