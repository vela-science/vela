import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const packageUrl = [
  new URL("../../package.json", import.meta.url),
  new URL("../../../package.json", import.meta.url),
].find((candidate) => existsSync(fileURLToPath(candidate)));
if (packageUrl === undefined) throw new Error("Canopus package.json is missing");
const packageJson = JSON.parse(readFileSync(packageUrl, "utf8")) as { version?: unknown };
const toolchainUrl = [
  new URL("../../toolchain.lock.json", import.meta.url),
  new URL("../../../toolchain.lock.json", import.meta.url),
].find((candidate) => existsSync(fileURLToPath(candidate)));
if (toolchainUrl === undefined) throw new Error("Canopus toolchain.lock.json is missing");
const toolchain = JSON.parse(readFileSync(toolchainUrl, "utf8")) as {
  schema?: unknown;
  vela?: { version?: unknown };
  codex?: { version?: unknown };
};

if (typeof packageJson.version !== "string" || packageJson.version.length === 0) {
  throw new Error("Canopus package version is missing");
}

export const CANOPUS_VERSION = packageJson.version;
if (toolchain.schema !== "canopus.toolchain-lock.v1") {
  throw new Error("Canopus toolchain lock schema is unsupported");
}
if (typeof toolchain.vela?.version !== "string" || toolchain.vela.version.length === 0) {
  throw new Error("Canopus Vela toolchain version is missing");
}
if (typeof toolchain.codex?.version !== "string" || toolchain.codex.version.length === 0) {
  throw new Error("Canopus Codex toolchain version is missing");
}

export const SUPPORTED_VELA_VERSION = toolchain.vela.version;
export const SUPPORTED_CODEX_VERSION = toolchain.codex.version;
