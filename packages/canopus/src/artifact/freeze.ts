import { createHash, randomUUID } from "node:crypto";
import { constants } from "node:fs";
import {
  chmod,
  link,
  lstat,
  mkdir,
  open,
  readdir,
  unlink,
} from "node:fs/promises";
import path from "node:path";

import type { FrozenArtifact } from "../contracts/candidate.js";
import { relativePathAt } from "../contracts/validation.js";

export class ArtifactError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "ArtifactError";
  }
}

export interface FrozenArtifactLocation {
  artifact: FrozenArtifact;
  frozenPath: string;
}

function below(root: string, child: string): boolean {
  const relative = path.relative(root, child);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

export async function hashRegularFile(file: string, maxBytes: number): Promise<{
  digest: string;
  bytes: number;
}> {
  const noFollow = "O_NOFOLLOW" in constants ? constants.O_NOFOLLOW : 0;
  const handle = await open(file, constants.O_RDONLY | noFollow);
  try {
    const before = await handle.stat();
    if (!before.isFile() || before.nlink !== 1) {
      throw new ArtifactError("artifact must be one singly linked regular file");
    }
    if (before.size > maxBytes) {
      throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
    }
    const hash = createHash("sha256");
    const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, Math.max(1, maxBytes + 1)));
    let bytes = 0;
    while (true) {
      const remaining = maxBytes + 1 - bytes;
      if (remaining <= 0) throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
      const { bytesRead } = await handle.read(buffer, 0, Math.min(buffer.length, remaining), null);
      if (bytesRead === 0) break;
      bytes += bytesRead;
      if (bytes > maxBytes) throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
      hash.update(buffer.subarray(0, bytesRead));
    }
    const after = await handle.stat();
    if (
      before.dev !== after.dev ||
      before.ino !== after.ino ||
      before.size !== after.size ||
      before.mtimeMs !== after.mtimeMs ||
      bytes !== before.size
    ) {
      throw new ArtifactError("artifact changed while it was being hashed");
    }
    return { digest: `sha256:${hash.digest("hex")}`, bytes };
  } finally {
    await handle.close();
  }
}

async function copyBounded(source: string, target: string, maxBytes: number): Promise<{
  digest: string;
  bytes: number;
}> {
  const noFollow = "O_NOFOLLOW" in constants ? constants.O_NOFOLLOW : 0;
  const sourceHandle = await open(source, constants.O_RDONLY | noFollow);
  const targetHandle = await open(
    target,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
    0o600,
  );
  try {
    const before = await sourceHandle.stat();
    if (!before.isFile() || before.nlink !== 1) {
      throw new ArtifactError("artifact must be one singly linked regular file");
    }
    if (before.size > maxBytes) throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
    const hash = createHash("sha256");
    const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, Math.max(1, maxBytes + 1)));
    let bytes = 0;
    while (true) {
      const remaining = maxBytes + 1 - bytes;
      if (remaining <= 0) throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
      const { bytesRead } = await sourceHandle.read(
        buffer,
        0,
        Math.min(buffer.length, remaining),
        null,
      );
      if (bytesRead === 0) break;
      bytes += bytesRead;
      if (bytes > maxBytes) throw new ArtifactError(`artifact exceeds ${maxBytes} bytes`);
      hash.update(buffer.subarray(0, bytesRead));
      let offset = 0;
      while (offset < bytesRead) {
        const write = await targetHandle.write(buffer, offset, bytesRead - offset, null);
        offset += write.bytesWritten;
      }
    }
    await targetHandle.sync();
    const after = await sourceHandle.stat();
    if (
      before.dev !== after.dev ||
      before.ino !== after.ino ||
      before.size !== after.size ||
      before.mtimeMs !== after.mtimeMs ||
      bytes !== before.size
    ) {
      throw new ArtifactError("artifact changed while it was being frozen");
    }
    return { digest: `sha256:${hash.digest("hex")}`, bytes };
  } finally {
    await Promise.all([sourceHandle.close(), targetHandle.close()]);
  }
}

export async function freezeArtifact(options: {
  sourceRoot: string;
  artifactRoot: string;
  path: string;
  kind: string;
  maxBytes: number;
}): Promise<FrozenArtifactLocation> {
  const relative = relativePathAt(options.path, "artifact.path");
  if (options.kind.length === 0 || options.kind.length > 128) {
    throw new ArtifactError("artifact kind length must be 1..128");
  }
  const sourceRoot = path.resolve(options.sourceRoot);
  const source = path.resolve(sourceRoot, relative);
  if (!below(sourceRoot, source)) {
    throw new ArtifactError("artifact path escapes the worker output root");
  }
  const sourceStat = await lstat(source);
  if (sourceStat.isSymbolicLink()) throw new ArtifactError("artifact path is a symbolic link");

  await mkdir(options.artifactRoot, { recursive: true, mode: 0o700 });
  const temporary = path.join(options.artifactRoot, `.freeze-${randomUUID()}`);
  let copied: { digest: string; bytes: number };
  try {
    copied = await copyBounded(source, temporary, options.maxBytes);
    const frozenPath = path.join(options.artifactRoot, copied.digest.slice(7));
    try {
      await link(temporary, frozenPath);
      await chmod(frozenPath, 0o444);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      const existing = await hashRegularFile(frozenPath, options.maxBytes);
      if (existing.digest !== copied.digest || existing.bytes !== copied.bytes) {
        throw new ArtifactError("existing content-addressed artifact does not match its name");
      }
    }
    await unlink(temporary);
    const frozen = await hashRegularFile(frozenPath, options.maxBytes);
    if (frozen.digest !== copied.digest || frozen.bytes !== copied.bytes) {
      throw new ArtifactError("frozen artifact failed its immediate digest check");
    }
    return {
      artifact: { path: relative, kind: options.kind, digest: copied.digest, bytes: copied.bytes },
      frozenPath,
    };
  } catch (error) {
    try {
      await unlink(temporary);
    } catch (unlinkError) {
      if ((unlinkError as NodeJS.ErrnoException).code !== "ENOENT") throw unlinkError;
    }
    throw error;
  }
}

export async function sealArtifactStore(root: string): Promise<void> {
  const visit = async (current: string): Promise<void> => {
    const stat = await lstat(current);
    if (stat.isSymbolicLink()) throw new ArtifactError("artifact store contains a symbolic link");
    if (stat.isDirectory()) {
      for (const child of await readdir(current)) await visit(path.join(current, child));
      await chmod(current, 0o555);
      return;
    }
    if (!stat.isFile() || stat.nlink !== 1) {
      throw new ArtifactError("artifact store contains a non-regular or linked file");
    }
    await chmod(current, 0o444);
  };
  await visit(root);
}

export async function verifyFrozenArtifact(
  frozen: FrozenArtifactLocation,
  maxBytes: number,
): Promise<void> {
  const observed = await hashRegularFile(frozen.frozenPath, maxBytes);
  if (observed.digest !== frozen.artifact.digest || observed.bytes !== frozen.artifact.bytes) {
    throw new ArtifactError(`frozen artifact ${frozen.artifact.path} no longer matches its record`);
  }
}

