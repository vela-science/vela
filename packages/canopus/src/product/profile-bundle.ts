import { constants } from "node:fs";
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { canonicalJson, sha256Bytes } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";
import {
  SUPPORTED_PRODUCT_PLATFORMS,
  listProductProfiles,
  loadProductProfile,
  loadProfileDraft,
  loadProfileResultContract,
  packagedProfileResource,
  type ProductPlatform,
} from "./profile.js";

interface PackedFile {
  path: string;
  sha256: string;
  size: number;
}

export interface ProfileValidation {
  schema: "canopus.profile-validation.v1";
  name: string;
  profile_sha256: string;
  draft_sha256: string;
  result_contract_sha256: string | null;
  platforms: Record<ProductPlatform, {
    worker_profile_sha256: string;
    verifier_capsule_sha256: string;
    verifier_image: string;
    verifier_platform: string;
  }>;
}

async function exactResource(relative: string, expected: string, label: string): Promise<PackedFile> {
  const file = await packagedProfileResource(relative, label);
  const bytes = await readBoundedRegularFile(file, 268_435_456);
  const observed = sha256Bytes(bytes);
  if (observed !== expected) throw new Error(`${label} root drifted: expected ${expected}, observed ${observed}`);
  return { path: relative, sha256: observed, size: bytes.length };
}

export async function validateProductProfile(name: string): Promise<ProfileValidation> {
  const profiles = await Promise.all(
    SUPPORTED_PRODUCT_PLATFORMS.map((platform) => loadProductProfile(name, { platform })),
  );
  const canonical = profiles[0];
  if (canonical === undefined) throw new Error("profile has no supported platform capsules");
  await loadProfileDraft(canonical);
  await loadProfileResultContract(canonical);
  const profileFile = await packagedProfileResource(`profiles/${name}.json`, "profile");
  const profileBytes = await readBoundedRegularFile(profileFile, 1024 * 1024);
  const platforms = Object.fromEntries(await Promise.all(profiles.map(async (profile) => {
    await exactResource(
      profile.permission_profile,
      profile.permission_profile_sha256,
      `${profile.platform} worker profile`,
    );
    await exactResource(
      profile.capsule_binary,
      profile.capsule_sha256,
      `${profile.platform} verifier capsule`,
    );
    return [profile.platform, {
      worker_profile_sha256: profile.permission_profile_sha256,
      verifier_capsule_sha256: profile.capsule_sha256,
      verifier_image: profile.verifier_image,
      verifier_platform: profile.verifier_platform,
    }];
  }))) as ProfileValidation["platforms"];
  return {
    schema: "canopus.profile-validation.v1",
    name,
    profile_sha256: sha256Bytes(profileBytes),
    draft_sha256: canonical.draft_sha256,
    result_contract_sha256: canonical.result_contract_sha256 ?? null,
    platforms,
  };
}

export async function packProductProfile(
  name: string,
  outputRoot: string,
): Promise<{ manifest: string; root: string; files: number }> {
  const validation = await validateProductProfile(name);
  const profile = await loadProductProfile(name, { platform: SUPPORTED_PRODUCT_PLATFORMS[0] });
  const resources = new Map<string, PackedFile>();
  const add = async (relative: string, expected: string, label: string): Promise<void> => {
    resources.set(relative, await exactResource(relative, expected, label));
  };
  await add(`profiles/${name}.json`, validation.profile_sha256, "profile");
  await add(profile.draft, profile.draft_sha256, "profile draft");
  if (profile.result_contract !== undefined && profile.result_contract_sha256 !== undefined) {
    await add(
      profile.result_contract,
      profile.result_contract_sha256,
      "profile result contract",
    );
  }
  for (const platform of SUPPORTED_PRODUCT_PLATFORMS) {
    const capsule = profile.platforms[platform];
    await add(capsule.worker_profile, capsule.worker_profile_sha256, `${platform} worker profile`);
    await add(capsule.verifier_capsule, capsule.verifier_capsule_sha256, `${platform} verifier capsule`);
  }

  await mkdir(outputRoot, { recursive: false, mode: 0o755 });
  for (const resource of [...resources.values()].sort((left, right) => left.path.localeCompare(right.path))) {
    const source = await packagedProfileResource(resource.path, "profile pack resource");
    const destination = path.join(outputRoot, resource.path);
    await mkdir(path.dirname(destination), { recursive: true, mode: 0o755 });
    await copyFile(source, destination, constants.COPYFILE_EXCL);
  }
  const manifestValue = {
    schema: "canopus.profile-pack.v1",
    profile: name,
    profile_sha256: validation.profile_sha256,
    files: [...resources.values()].sort((left, right) => left.path.localeCompare(right.path)),
  };
  const manifestBytes = canonicalJson(manifestValue);
  const manifest = path.join(outputRoot, "profile-pack.json");
  await writeFile(manifest, manifestBytes, { encoding: "utf8", flag: "wx", mode: 0o644 });
  return { manifest, root: sha256Bytes(manifestBytes), files: resources.size };
}

export { listProductProfiles };
