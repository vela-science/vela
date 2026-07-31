import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const output = path.join(root, "dist", "vela-agent");
const [engineOutputSchema, macosPermissionProfile, linuxPermissionProfile] =
  await Promise.all([
    readFile(path.join(root, "schemas", "engine-output.v0.json"), "utf8"),
    readFile(path.join(root, "runtime", "native-worker", "config.toml"), "utf8"),
    readFile(path.join(root, "runtime", "native-worker", "config-linux.toml"), "utf8"),
  ]);

const buildOptions = {
  entrypoints: [path.join(root, "src", "agent-cli.ts")],
  outdir: path.dirname(output),
  naming: { entry: path.basename(output) },
  target: "bun",
  format: "esm",
  minify: false,
  sourcemap: "none",
  write: false,
  define: {
    __VELA_AGENT_ENGINE_OUTPUT_SCHEMA__: JSON.stringify(engineOutputSchema),
    __VELA_AGENT_MACOS_PERMISSION_PROFILE__: JSON.stringify(macosPermissionProfile),
    __VELA_AGENT_LINUX_PERMISSION_PROFILE__: JSON.stringify(linuxPermissionProfile),
  },
};

async function buildOnce() {
  const result = await Bun.build(buildOptions);
  if (!result.success) {
    for (const log of result.logs) process.stderr.write(`${log}\n`);
    process.exit(1);
  }
  if (result.outputs.length !== 1 || result.outputs[0]?.kind !== "entry-point") {
    throw new Error(`Agent helper build emitted ${result.outputs.length} outputs instead of one bundle`);
  }
  return Buffer.from(await result.outputs[0].arrayBuffer());
}

const first = await buildOnce();
const second = await buildOnce();
if (!first.equals(second)) {
  throw new Error("Agent helper build is not byte-deterministic");
}
if (first.subarray(0, 2).toString("utf8") === "#!") {
  throw new Error("Agent helper bundle must be invoked by the exact rooted Bun runtime, not a shebang");
}
await mkdir(path.dirname(output), { recursive: true });
await rm(output, { force: true });
await writeFile(output, first, { flag: "wx", mode: 0o644 });
