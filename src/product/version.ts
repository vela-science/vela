import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const packageUrl = [
  new URL("../../package.json", import.meta.url),
  new URL("../../../package.json", import.meta.url),
].find((candidate) => existsSync(fileURLToPath(candidate)));
if (packageUrl === undefined) throw new Error("Canopus package.json is missing");
const packageJson = JSON.parse(readFileSync(packageUrl, "utf8")) as { version?: unknown };

if (typeof packageJson.version !== "string" || packageJson.version.length === 0) {
  throw new Error("Canopus package version is missing");
}

export const CANOPUS_VERSION = packageJson.version;
export const SUPPORTED_VELA_VERSION = "0.940.6";
export const SUPPORTED_CODEX_VERSION = "0.145.0";
