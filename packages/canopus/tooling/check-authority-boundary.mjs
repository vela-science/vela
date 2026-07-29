#!/usr/bin/env node

// Repository-only package boundary check; excluded from the published product.
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../../..");
const canopusSource = path.join(root, "packages", "canopus", "src");
const forbidden = [
  /from\s+["'][^"']*crates\//u,
  /from\s+["'][^"']*vela-authority/u,
  /from\s+["'][^"']*current_repository_decision/u,
  /from\s+["'][^"']*repository_authority/u,
];

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesBelow(candidate);
    return statSync(candidate).isFile() && candidate.endsWith(".ts") ? [candidate] : [];
  });
}

for (const file of filesBelow(canopusSource)) {
  const source = readFileSync(file, "utf8");
  for (const pattern of forbidden) {
    if (pattern.test(source)) {
      throw new Error(`${path.relative(root, file)} crosses the Canopus authority boundary`);
    }
  }
}

process.stdout.write("package-boundary checks passed\n");
