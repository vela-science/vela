#!/usr/bin/env node

import process from "node:process";
import { fileURLToPath } from "node:url";

import { runNativeCustodyPreflight } from "../dist/src/product/custody.js";

const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== "--codex") {
  throw new Error("usage: run-native-sandbox-preflight.mjs --codex BIN");
}

const result = await runNativeCustodyPreflight({
  binary: args[1],
  permissionProfile: fileURLToPath(new URL(
    process.platform === "linux"
      ? "../runtime/native-worker/config-linux.toml"
      : "../runtime/native-worker/config.toml",
    import.meta.url,
  )),
});
process.stdout.write(`${JSON.stringify(result)}\n`);
