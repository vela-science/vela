#!/usr/bin/env node

import { readFileSync } from "node:fs";

const lock = JSON.parse(
  readFileSync(new URL("../toolchain.lock.json", import.meta.url), "utf8"),
);

const requested = process.argv[2] ?? process.platform;
const platform = {
  Linux: "linux-x64",
  macOS: "darwin-arm64",
  Windows: "win32-x64",
  linux: "linux-x64",
  darwin: "darwin-arm64",
  win32: "win32-x64",
}[requested];

if (platform === undefined) {
  throw new Error(`unsupported toolchain platform ${requested}`);
}

const asset = lock.vela.assets[platform];
if (asset === undefined) {
  throw new Error(`toolchain lock has no Vela asset for ${platform}`);
}

const values = {
  VELA_VERSION: lock.vela.version,
  VELA_TAG: lock.vela.tag,
  VELA_ARCHIVE: asset.archive,
  VELA_ARCHIVE_SHA256: asset.archive_sha256,
  VELA_SHA256: asset.binary_sha256,
  VELA_URL: `${lock.vela.release_base_url}/${asset.archive}`,
  CODEX_VERSION: lock.codex.version,
  CODEX_TARBALL_SHA256: lock.codex.linux_x64_tarball_sha256,
  CODEX_BINARY_SHA256: lock.codex.linux_x64_binary_sha256,
};

for (const [key, value] of Object.entries(values)) {
  if (typeof value !== "string" || value.includes("\n")) {
    throw new Error(`invalid ${key} in toolchain lock`);
  }
  process.stdout.write(`${key}=${value}\n`);
}
