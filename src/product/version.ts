import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("../../../package.json", import.meta.url), "utf8"),
) as { version?: unknown };

if (typeof packageJson.version !== "string" || packageJson.version.length === 0) {
  throw new Error("Canopus package version is missing");
}

export const CANOPUS_VERSION = packageJson.version;
export const SUPPORTED_VELA_VERSION = "0.930.0-rc.9";
export const SUPPORTED_CODEX_VERSION = "0.145.0";
