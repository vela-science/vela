import path from "node:path";
import { mkdir, rm, writeFile } from "node:fs/promises";

import { readBoundedRegularFile } from "../util/files.js";

const AUTH_MAX_BYTES = 2 * 1024 * 1024;
const MODEL_CATALOG_MAX_BYTES = 32 * 1024 * 1024;

async function optionalBoundedFile(file: string, maxBytes: number): Promise<Buffer | undefined> {
  try {
    return await readBoundedRegularFile(file, maxBytes);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw error;
  }
}

/**
 * Stage only the credential and optional model catalog needed by Codex into a
 * fresh, disposable home. Native Codex writes installation_id during startup;
 * it must never receive write access to the user's real credential directory.
 */
export async function prepareIsolatedCodexHome(
  sourceHome: string,
  runtimeRoot: string,
  config?: Uint8Array,
): Promise<string> {
  const source = path.resolve(sourceHome);
  const destination = path.join(path.resolve(runtimeRoot), "codex-runtime");
  await mkdir(destination, { mode: 0o700 });
  try {
    const auth = await readBoundedRegularFile(path.join(source, "auth.json"), AUTH_MAX_BYTES);
    const modelCatalog = await optionalBoundedFile(
      path.join(source, "models_cache.json"),
      MODEL_CATALOG_MAX_BYTES,
    );
    await writeFile(path.join(destination, "auth.json"), auth, { flag: "wx", mode: 0o600 });
    if (modelCatalog !== undefined) {
      await writeFile(path.join(destination, "models_cache.json"), modelCatalog, {
        flag: "wx",
        mode: 0o600,
      });
    }
    if (config !== undefined) {
      await writeFile(path.join(destination, "config.toml"), config, {
        flag: "wx",
        mode: 0o600,
      });
    }
    return destination;
  } catch (error) {
    await rm(destination, { recursive: true, force: true });
    throw error;
  }
}

export async function removeIsolatedCodexHome(codexHome: string): Promise<void> {
  await rm(codexHome, { recursive: true, force: true });
}
