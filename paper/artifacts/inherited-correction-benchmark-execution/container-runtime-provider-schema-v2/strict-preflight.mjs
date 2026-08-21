#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { configArgs, STRICT_OVERRIDES } from "./strict-config.mjs";

const sha = data => `sha256:${createHash("sha256").update(data).digest("hex")}`;
const legacy = process.argv.includes("--legacy-unsupported");
const outputIndex = process.argv.indexOf("--output");
const output = outputIndex >= 0 ? process.argv[outputIndex + 1] : null;
if (outputIndex >= 0 && !output) throw new Error("missing_output_directory");

const result = spawnSync(
  "codex",
  ["app-server", "--strict-config", ...configArgs(legacy), "--listen", "stdio://"],
  { input: "", encoding: "utf8", env: { CODEX_HOME: "/codex-home", PATH: process.env.PATH, RUST_LOG: "off" } },
);
const expectedSuccess = !legacy;
const passed = expectedSuccess
  ? result.status === 0 && result.stdout.length === 0
  : result.status !== 0 && result.stdout.length === 0 && result.stderr.includes("unknown configuration field `tools.view_image`");

if (output) {
  mkdirSync(output, { recursive: true });
  writeFileSync(join(output, "provider-events.jsonl"), "", { flag: "wx", mode: 0o600 });
  writeFileSync(join(output, "strict-stdout.txt"), result.stdout, { flag: "wx", mode: 0o600 });
  writeFileSync(join(output, "strict-stderr.txt"), result.stderr, { flag: "wx", mode: 0o600 });
  const receipt = {
    schema: "vela.inherited-correction-strict-config-preflight.v1",
    codex_cli_version: "0.149.0",
    mode: legacy ? "legacy_unsupported_regression" : "corrected_strict_configuration",
    expected_success: expectedSuccess,
    process_exit_code: result.status,
    strict_parse_passed: passed,
    provider_contact_possible: false,
    container_network: "none",
    provider_events_bytes: sha(Buffer.alloc(0)),
    stdout_bytes: sha(Buffer.from(result.stdout)),
    stderr_bytes: sha(Buffer.from(result.stderr)),
    overrides: legacy ? [...STRICT_OVERRIDES, "tools.view_image=false"] : STRICT_OVERRIDES,
  };
  writeFileSync(join(output, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx", mode: 0o600 });
}

process.stderr.write(result.stderr);
if (!passed) process.exit(2);
