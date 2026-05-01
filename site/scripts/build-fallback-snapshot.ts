#!/usr/bin/env bun
/*
  Build-time fallback snapshot for the workbench.

  The marketing-site /workbench page calls /api/frontier (proxied to the
  vela-workbench binary) to populate its constellation + table. When the
  binary is offline or unreachable, the page should still render
  *something*, not a single empty-table row reading "Failed to load."

  This script copies the canonical Alzheimer's Therapeutics frontier
  JSON from the protocol side (frontiers/alzheimers-therapeutics.json,
  ~1MB) into site/public/fallback-frontier.json so the page can fall
  back to a built-in static snapshot. The shape matches /api/frontier
  one-to-one — same `frontier`, `findings`, `events`, `proposals`,
  `sources` keys — so the page's loader doesn't need a second branch.

  Wired into package.json `prebuild`. Re-runs on every astro build.
*/

import { copyFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const siteRoot = dirname(here);
const repoRoot = dirname(siteRoot);

const SOURCE = join(repoRoot, "frontiers", "alzheimers-therapeutics.json");
const TARGET_DIR = join(siteRoot, "public");
const TARGET = join(TARGET_DIR, "fallback-frontier.json");

if (!existsSync(SOURCE)) {
  console.error(
    `[fallback-snapshot] source frontier missing at ${SOURCE}\n` +
      `  ensure the protocol-side build has run; the workbench page\n` +
      `  will fall through to its current "unreachable" error state.`,
  );
  process.exit(1);
}

if (!existsSync(TARGET_DIR)) mkdirSync(TARGET_DIR, { recursive: true });

copyFileSync(SOURCE, TARGET);
const bytes = statSync(TARGET).size;
console.log(
  `[fallback-snapshot] wrote ${TARGET} (${(bytes / 1024).toFixed(1)} KB)`,
);
