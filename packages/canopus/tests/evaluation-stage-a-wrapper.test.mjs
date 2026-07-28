import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const sha256 = (bytes) =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

test("Stage A wrapper never passes its supervisor channel to the model", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "canopus-stage-a-wrapper-"));
  try {
    const wrapper = path.join(directory, "stage-a.mjs");
    const build = spawnSync(
      "bun",
      [
        path.join(packageRoot, "evaluation/wrappers/build-stage-a.mjs"),
        "--output",
        wrapper,
      ],
      { cwd: packageRoot, encoding: "utf8" },
    );
    assert.equal(build.status, 0, build.stderr);

    const fakeCodex = path.join(directory, "codex");
    writeFileSync(fakeCodex, [
      "#!/usr/bin/env node",
      "const fs=require('node:fs');",
      "const path=require('node:path');",
      "const args=process.argv.slice(2);",
      "if(args[0]==='--version'){process.stdout.write('codex-cli fixture\\n');process.exit(0);}",
      "if(args[0]==='sandbox'){process.stdout.write(" +
        "'true false false false false false false false false false\\n');process.exit(0);}",
      "if(args[0]!=='exec')process.exit(64);",
      "const finalPath=args[args.indexOf('--output-last-message')+1];",
      "let fd3='closed';",
      "try{fs.writeSync(3,'model-forged-control\\n');fd3='open';}catch{}",
      "fs.mkdirSync(path.join(process.cwd(),'artifacts'),{recursive:true});",
      "fs.writeFileSync(path.join(process.cwd(),'artifacts/result.txt'),`fd3=${fd3}\\n`);",
      "fs.writeFileSync(finalPath,JSON.stringify({",
      "schema:'canopus.engine-output.v0',status:'success',",
      "claim:'One bounded fixture artifact was produced.',",
      "artifacts:[{path:'artifacts/result.txt',kind:'text/plain',encoding:'utf8',content:''}],",
      "observations:['The fixture ran once.'],",
      "caveats:['Verification and acceptance remain separate.']}));",
      "for(const event of [",
      "{type:'thread.started',thread_id:'fixture'},",
      "{type:'turn.started'},",
      "{type:'item.completed',item:{id:'command',type:'command_execution',command:'printf candidate'}},",
      "{type:'turn.completed',usage:{input_tokens:10,cached_input_tokens:0," +
        "output_tokens:5,reasoning_output_tokens:2}}",
      "])process.stdout.write(JSON.stringify(event)+'\\n');",
    ].join("\n"));
    chmodSync(fakeCodex, 0o700);

    const authHome = path.join(directory, "auth");
    mkdirSync(authHome);
    writeFileSync(path.join(authHome, "auth.json"), JSON.stringify({
      OPENAI_API_KEY: null,
      tokens: {
        access_token: "fixture-access-token-000000",
        id_token: "fixture-id-token-0000000000",
        refresh_token: "fixture-refresh-token-0000",
      },
    }), { mode: 0o600 });
    const permissionProfile = path.join(directory, "config.toml");
    writeFileSync(permissionProfile, [
      'default_permissions = "canopus-worker"',
      "approval_policy = \"never\"",
      "[permissions.canopus-worker.filesystem]",
      '":minimal" = "read"',
      '[permissions.canopus-worker.filesystem.":workspace_roots"]',
      '"." = "write"',
      "[permissions.canopus-worker.network]",
      "enabled = false",
    ].join("\n") + "\n");
    const outputSchema = path.join(directory, "engine-output.json");
    writeFileSync(
      outputSchema,
      readFileSync(path.join(packageRoot, "schemas/engine-output.v0.json")),
    );
    const packet = path.join(directory, "packet.json");
    writeFileSync(packet, `${JSON.stringify({
      schema: "canopus.evaluation-task-packet.v1",
      task_id: "fixture:task",
      objective: "Write the bounded fixture artifact.",
      output: { path: "artifacts/result.txt" },
    })}\n`);
    const output = path.join(directory, "assignment");
    mkdirSync(output);
    const run = spawnSync(
      "bun",
      [
        wrapper,
        "--mode",
        "native_packet",
        "--task-packet",
        packet,
        "--output",
        output,
        "--assignment",
        "A-fixture-native-r1",
        "--seed",
        "1",
        "--codex",
        fakeCodex,
        "--codex-version",
        "codex-cli fixture",
        "--codex-root",
        sha256(readFileSync(fakeCodex)),
        "--model",
        "fixture-model",
        "--permission-profile",
        permissionProfile,
        "--output-schema",
        outputSchema,
        "--max-wall-ms",
        "10000",
        "--max-tokens",
        "1000",
        "--max-artifact-bytes",
        "1024",
      ],
      {
        cwd: packageRoot,
        env: { ...process.env, CODEX_HOME: authHome },
        stdio: ["ignore", "pipe", "pipe", "pipe"],
        encoding: "buffer",
      },
    );
    assert.equal(run.status, 0, run.stderr?.toString("utf8"));
    assert.equal(
      readFileSync(path.join(output, "artifacts/result.txt"), "utf8"),
      "fd3=closed\n",
    );
    const control = JSON.parse(run.output[3].toString("utf8"));
    assert.equal(control.assignment_id, "A-fixture-native-r1");
    assert.deepEqual(control.usage, {
      input_tokens: 10,
      cached_input_tokens: 0,
      output_tokens: 5,
      reasoning_output_tokens: 2,
    });
    assert.equal(
      existsSync(path.join(output, "producer/worker-events.jsonl")),
      false,
    );
    assert.equal(run.output[3].includes(Buffer.from("model-forged-control")), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

