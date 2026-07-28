import { createHash } from "node:crypto";
import { constants, type Stats } from "node:fs";
import { open } from "node:fs/promises";

export const MAX_EXECUTABLE_BYTES = 1024 * 1024 * 1024;

function assertStableRegularFile(
  file: string,
  before: Stats,
  after: Stats,
): void {
  if (
    before.dev !== after.dev ||
    before.ino !== after.ino ||
    before.size !== after.size ||
    before.mtimeMs !== after.mtimeMs
  ) {
    throw new Error(`${file} changed while it was being read`);
  }
}

export async function readBoundedRegularFile(file: string, maxBytes: number): Promise<Buffer> {
  const noFollow = "O_NOFOLLOW" in constants ? constants.O_NOFOLLOW : 0;
  const handle = await open(file, constants.O_RDONLY | noFollow);
  try {
    const before = await handle.stat();
    if (!before.isFile() || before.nlink !== 1) {
      throw new Error(`${file} is not one singly linked regular file`);
    }
    if (before.size > maxBytes) throw new Error(`${file} exceeds ${maxBytes} bytes`);
    const value = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < value.length) {
      const { bytesRead } = await handle.read(value, offset, value.length - offset, offset);
      if (bytesRead === 0) throw new Error(`${file} ended before its declared size`);
      offset += bytesRead;
    }
    assertStableRegularFile(file, before, await handle.stat());
    return value;
  } finally {
    await handle.close();
  }
}

export async function sha256RegularFile(file: string, maxBytes: number): Promise<string> {
  const noFollow = "O_NOFOLLOW" in constants ? constants.O_NOFOLLOW : 0;
  const handle = await open(file, constants.O_RDONLY | noFollow);
  try {
    const before = await handle.stat();
    if (!before.isFile() || before.nlink !== 1) {
      throw new Error(`${file} is not one singly linked regular file`);
    }
    if (before.size > maxBytes) throw new Error(`${file} exceeds ${maxBytes} bytes`);
    const hash = createHash("sha256");
    const buffer = Buffer.allocUnsafe(1024 * 1024);
    let offset = 0;
    while (offset < before.size) {
      const length = Math.min(buffer.length, before.size - offset);
      const { bytesRead } = await handle.read(buffer, 0, length, offset);
      if (bytesRead === 0) throw new Error(`${file} ended before its declared size`);
      hash.update(buffer.subarray(0, bytesRead));
      offset += bytesRead;
    }
    assertStableRegularFile(file, before, await handle.stat());
    return `sha256:${hash.digest("hex")}`;
  } finally {
    await handle.close();
  }
}
