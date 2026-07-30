import { spawn } from "node:child_process";
import { lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";

import {
  SOURCE_CHECKOUT_COMMIT,
  SOURCE_CHECKOUT_TREE,
  SOURCE_REPOSITORY,
  sha256,
} from "./task.mjs";

export function safeRelativePath(value) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.startsWith("/") ||
    value.includes("\\") ||
    value.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
  ) {
    throw new Error(`unsafe source-relative path: ${String(value)}`);
  }
  return value;
}

export async function readBoundFile(root, relative, expectedRoot, maxBytes) {
  const safe = safeRelativePath(relative);
  const candidate = path.resolve(root, safe);
  const resolved = await realpath(candidate);
  if (resolved !== candidate) {
    throw new Error(`source path substitution detected for ${safe}`);
  }
  const metadata = await lstat(candidate);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.size === 0 ||
    metadata.size > maxBytes
  ) {
    throw new Error(`source file ${safe} violates its file contract`);
  }
  const bytes = await readFile(candidate);
  const observedRoot = sha256(bytes);
  if (observedRoot !== expectedRoot) {
    throw new Error(
      `source file ${safe} root drifted: expected ${expectedRoot}, observed ${observedRoot}`,
    );
  }
  return bytes;
}

function execute(argv, maxBytes = 1024 * 1024, timeoutMs = 30_000) {
  return new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      env: {
        PATH: "/usr/bin:/bin",
        LANG: "C",
        LC_ALL: "C",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    let settled = false;
    const finish = (callback) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      callback();
    };
    const collect = (target, chunk) => {
      bytes += chunk.length;
      if (bytes > maxBytes) {
        child.kill("SIGKILL");
        finish(() => reject(new Error("source inspection exceeded its output bound")));
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", (error) => finish(() => reject(error)));
    child.on("close", (code) => finish(() => {
      if (code !== 0) {
        reject(new Error(
          `${argv[0]} exited ${String(code)}: ${Buffer.concat(stderr).toString("utf8").trim()}`,
        ));
        return;
      }
      resolve(Buffer.concat(stdout).toString("utf8").trim());
    }));
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish(() => reject(new Error("source inspection timed out")));
    }, timeoutMs);
  });
}

export function assertCheckoutIdentity({ commit, tree, origin, status }) {
  if (commit !== SOURCE_CHECKOUT_COMMIT) {
    throw new Error(
      `Erdős checkout commit drifted: expected ${SOURCE_CHECKOUT_COMMIT}, observed ${commit}`,
    );
  }
  if (tree !== SOURCE_CHECKOUT_TREE) {
    throw new Error(
      `Erdős checkout tree drifted: expected ${SOURCE_CHECKOUT_TREE}, observed ${tree}`,
    );
  }
  if (origin !== SOURCE_REPOSITORY) {
    throw new Error(
      `Erdős checkout origin drifted: expected ${SOURCE_REPOSITORY}, observed ${origin}`,
    );
  }
  if (status !== "") {
    throw new Error("Erdős checkout is dirty");
  }
}

export async function inspectCheckout(frontier) {
  const git = "/usr/bin/git";
  const [commit, tree, origin, status] = await Promise.all([
    execute([git, "-C", frontier, "rev-parse", "HEAD"]),
    execute([git, "-C", frontier, "rev-parse", "HEAD^{tree}"]),
    execute([git, "-C", frontier, "remote", "get-url", "origin"]),
    execute([
      git,
      "-C",
      frontier,
      "status",
      "--porcelain=v1",
      "--untracked-files=all",
    ]),
  ]);
  const identity = { commit, tree, origin, status };
  assertCheckoutIdentity(identity);
  return identity;
}

export function assertOutputOutsideFrontier(frontier, output) {
  const relative = path.relative(frontier, output);
  if (relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))) {
    throw new Error("task output must remain outside the canonical Frontier checkout");
  }
}
