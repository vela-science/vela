#!/usr/bin/env bun

// Source-only evaluation command; excluded from the npm payload.
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  canonicalJson,
  digest,
  parseEvaluationRun,
} from "../lib/evaluation-plan.mjs";

function values(argv) {
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!["--input", "--output", "--format", "--content"].includes(key) || value === undefined) {
      throw new Error(`invalid trace option near ${key ?? "end"}`);
    }
    result.set(key, value);
  }
  return result;
}

const options = values(process.argv.slice(2).filter((value) => value !== "--"));
if (options.get("--format") !== "otlp-json" || options.get("--content") !== "none") {
  throw new Error("trace export supports only --format otlp-json --content none");
}
const input = parseEvaluationRun(
  JSON.parse(await readFile(path.resolve(options.get("--input")), "utf8")),
);
const trace = {
  resourceSpans: [{
    resource: {
      attributes: [
        { key: "service.name", value: { stringValue: "canopus-evaluation" } },
        { key: "canopus.plan_root", value: { stringValue: input.plan_root } },
      ],
    },
    scopeSpans: [{
      scope: { name: "canopus.source-only-trace-export", version: "1" },
      spans: [{
        traceId: digest({ run_id: input.run_id }).slice(7, 39),
        spanId: digest({ assignment: input.assignment.id }).slice(7, 23),
        name: `${input.assignment.task_id}/${input.assignment.arm_id}`,
        startTimeUnixNano: String(BigInt(Date.parse(input.started_at)) * 1_000_000n),
        endTimeUnixNano: String(BigInt(Date.parse(input.ended_at)) * 1_000_000n),
        attributes: [
          { key: "canopus.run_id", value: { stringValue: input.run_id } },
          { key: "canopus.stage", value: { stringValue: input.assignment.stage } },
          { key: "canopus.task_id", value: { stringValue: input.assignment.task_id } },
          { key: "canopus.arm_id", value: { stringValue: input.assignment.arm_id } },
          { key: "process.exit_code", value: { intValue: String(input.exit_code ?? -1) } },
          { key: "process.stdout_root", value: { stringValue: input.stdout_root } },
          { key: "process.stderr_root", value: { stringValue: input.stderr_root } },
          { key: "vela.authority_effect", value: { stringValue: "none" } },
          { key: "gen_ai.content_recorded", value: { boolValue: false } },
        ],
        status: {
          code:
            input.exit_code === 0 && input.runner_error === null && !input.timed_out
              ? 1
              : 2,
        },
      }],
    }],
  }],
};
const output = path.resolve(options.get("--output"));
await writeFile(output, canonicalJson(trace), { mode: 0o600, flag: "wx" });
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "trace:export",
  output,
  trace_root: digest(trace),
  content: "none",
})}\n`);
