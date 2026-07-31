import { randomUUID } from "node:crypto";
import { lstat, mkdir, readdir, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { TextDecoder } from "node:util";

import { canonicalJson, sha256Bytes } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";

const STAGING_DIRECTORY = ".worker-evidence-staging";
const FAILURE_DIRECTORY = "failure-evidence";
const MAX_SCANNED_ENTRIES = 4096;
const MAX_RETAINED_FILES = 16;
const MAX_FILE_BYTES = 64 * 1024;
const MAX_TOTAL_BYTES = 256 * 1024;
const SOURCE_EXTENSIONS = new Set([
  ".c",
  ".cc",
  ".cpp",
  ".cxx",
  ".h",
  ".hh",
  ".hpp",
  ".hxx",
  ".jl",
  ".js",
  ".jsx",
  ".lean",
  ".m",
  ".mjs",
  ".mts",
  ".py",
  ".r",
  ".rs",
  ".sh",
  ".sql",
  ".ts",
  ".tsx",
]);
const BUILD_FILES = new Set([
  "CMakeLists.txt",
  "Cargo.toml",
  "Makefile",
  "meson.build",
  "pyproject.toml",
]);

interface OmittedCounts {
  baseline_or_contract: number;
  unsupported_type: number;
  non_regular: number;
  oversized: number;
  non_utf8: number;
  sensitive: number;
  file_limit: number;
  total_limit: number;
  scan_limit: number;
}

export interface WorkerWorkspaceEvidence {
  schema: "canopus.worker-workspace-evidence.v1";
  authority: "non_authoritative";
  scope: "new_source_and_build_files";
  limits: {
    max_scanned_entries: number;
    max_retained_files: number;
    max_file_bytes: number;
    max_total_bytes: number;
  };
  retained: Array<{
    path: string;
    digest: string;
    bytes: number;
  }>;
  omitted: OmittedCounts;
}

export interface PromotedWorkerWorkspaceEvidence {
  path: "failure-evidence/manifest.json";
  root: string;
  retained_files: number;
  retained_bytes: number;
}

function isBelow(relative: string): boolean {
  return relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative);
}

function containsSensitiveMaterial(bytes: Buffer, secrets: readonly Buffer[]): boolean {
  if (secrets.some((secret) => secret.length > 0 && bytes.includes(secret))) return true;
  const text = bytes.toString("utf8");
  return (
    /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----/u.test(text) ||
    /\bsk-[A-Za-z0-9_-]{20,}\b/u.test(text) ||
    /\bgh[pousr]_[A-Za-z0-9]{20,}\b/u.test(text) ||
    /\bAKIA[0-9A-Z]{16}\b/u.test(text)
  );
}

function isSourceOrBuildFile(relative: string): boolean {
  const basename = path.basename(relative);
  return BUILD_FILES.has(basename) || SOURCE_EXTENSIONS.has(path.extname(basename).toLowerCase());
}

async function listEntries(root: string): Promise<{
  files: string[];
  scanLimitReached: boolean;
}> {
  const files: string[] = [];
  let entries = 0;
  let scanLimitReached = false;

  const visit = async (directory: string): Promise<void> => {
    if (scanLimitReached) return;
    const children = (await readdir(directory)).sort();
    for (const child of children) {
      if (scanLimitReached) break;
      entries += 1;
      if (entries > MAX_SCANNED_ENTRIES) {
        scanLimitReached = true;
        break;
      }
      const absolute = path.join(directory, child);
      const relative = path.relative(root, absolute);
      if (!isBelow(relative)) continue;
      const stat = await lstat(absolute);
      if (stat.isDirectory()) {
        await visit(absolute);
      } else {
        files.push(relative);
      }
    }
  };

  await visit(root);
  return { files, scanLimitReached };
}

export async function snapshotWorkerWorkspace(workspace: string): Promise<Set<string>> {
  return new Set((await listEntries(workspace)).files);
}

export async function stageWorkerWorkspaceEvidence(options: {
  workspace: string;
  runRoot: string;
  baseline: ReadonlySet<string>;
  excludedPaths: readonly string[];
  secrets: readonly Buffer[];
}): Promise<WorkerWorkspaceEvidence> {
  const workspace = path.resolve(options.workspace);
  const staging = path.join(options.runRoot, STAGING_DIRECTORY);
  const temporary = path.join(options.runRoot, `.worker-evidence-${randomUUID()}`);
  const excluded = new Set(options.excludedPaths);
  const omitted: OmittedCounts = {
    baseline_or_contract: 0,
    unsupported_type: 0,
    non_regular: 0,
    oversized: 0,
    non_utf8: 0,
    sensitive: 0,
    file_limit: 0,
    total_limit: 0,
    scan_limit: 0,
  };
  const retained: WorkerWorkspaceEvidence["retained"] = [];
  let totalBytes = 0;

  await mkdir(path.join(temporary, "files"), { recursive: true, mode: 0o700 });
  try {
    const listing = await listEntries(workspace);
    omitted.scan_limit = listing.scanLimitReached ? 1 : 0;
    for (const relative of listing.files) {
      if (
        options.baseline.has(relative) ||
        excluded.has(relative) ||
        relative === ".canopus-final.json" ||
        relative === ".canopus-runtime" ||
        relative.startsWith(`.canopus-runtime${path.sep}`)
      ) {
        omitted.baseline_or_contract += 1;
        continue;
      }
      if (!isSourceOrBuildFile(relative)) {
        omitted.unsupported_type += 1;
        continue;
      }
      if (retained.length >= MAX_RETAINED_FILES) {
        omitted.file_limit += 1;
        continue;
      }

      const absolute = path.join(workspace, relative);
      const stat = await lstat(absolute);
      if (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1) {
        omitted.non_regular += 1;
        continue;
      }
      if (stat.size > MAX_FILE_BYTES) {
        omitted.oversized += 1;
        continue;
      }
      if (totalBytes + stat.size > MAX_TOTAL_BYTES) {
        omitted.total_limit += 1;
        continue;
      }

      const bytes = await readBoundedRegularFile(absolute, MAX_FILE_BYTES);
      try {
        new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      } catch {
        omitted.non_utf8 += 1;
        continue;
      }
      if (containsSensitiveMaterial(bytes, options.secrets)) {
        omitted.sensitive += 1;
        continue;
      }

      const target = path.join(temporary, "files", relative);
      const targetRelative = path.relative(path.join(temporary, "files"), target);
      if (!isBelow(targetRelative)) {
        omitted.non_regular += 1;
        continue;
      }
      await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
      await writeFile(target, bytes, { flag: "wx", mode: 0o600 });
      totalBytes += bytes.length;
      retained.push({
        path: relative,
        digest: sha256Bytes(bytes),
        bytes: bytes.length,
      });
    }

    const manifest: WorkerWorkspaceEvidence = {
      schema: "canopus.worker-workspace-evidence.v1",
      authority: "non_authoritative",
      scope: "new_source_and_build_files",
      limits: {
        max_scanned_entries: MAX_SCANNED_ENTRIES,
        max_retained_files: MAX_RETAINED_FILES,
        max_file_bytes: MAX_FILE_BYTES,
        max_total_bytes: MAX_TOTAL_BYTES,
      },
      retained,
      omitted,
    };
    await writeFile(path.join(temporary, "manifest.json"), canonicalJson(manifest), {
      flag: "wx",
      mode: 0o600,
    });
    await rename(temporary, staging);
    return manifest;
  } catch (error) {
    await rm(temporary, { recursive: true, force: true }).catch(() => undefined);
    throw error;
  }
}

export async function promoteWorkerWorkspaceEvidence(
  runRoot: string,
): Promise<PromotedWorkerWorkspaceEvidence | null> {
  const staging = path.join(runRoot, STAGING_DIRECTORY);
  const destination = path.join(runRoot, FAILURE_DIRECTORY);
  let manifestBytes: Buffer;
  try {
    manifestBytes = await readBoundedRegularFile(
      path.join(staging, "manifest.json"),
      64 * 1024,
    );
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
  const manifest = JSON.parse(manifestBytes.toString("utf8")) as WorkerWorkspaceEvidence;
  if (
    manifest.schema !== "canopus.worker-workspace-evidence.v1" ||
    manifest.authority !== "non_authoritative" ||
    !Array.isArray(manifest.retained)
  ) {
    throw new Error("worker workspace evidence manifest is malformed");
  }
  await rename(staging, destination);
  return {
    path: "failure-evidence/manifest.json",
    root: sha256Bytes(manifestBytes),
    retained_files: manifest.retained.length,
    retained_bytes: manifest.retained.reduce((sum, entry) => sum + entry.bytes, 0),
  };
}

export async function discardWorkerWorkspaceEvidence(runRoot: string): Promise<void> {
  await rm(path.join(runRoot, STAGING_DIRECTORY), { recursive: true, force: true });
}

export async function readWorkerWorkspaceEvidence(
  runRoot: string,
): Promise<WorkerWorkspaceEvidence> {
  const bytes = await readBoundedRegularFile(
    path.join(runRoot, FAILURE_DIRECTORY, "manifest.json"),
    64 * 1024,
  );
  return JSON.parse(bytes.toString("utf8")) as WorkerWorkspaceEvidence;
}
