import assert from "node:assert/strict";
import test from "node:test";

import {
  commandTraceArgv,
  reportedCommandMatchesTrace,
  tokenizeShellCommand,
} from "../scripts/lib/command-trace.mjs";

test("adjacent shell quotes normalize to the same argv token", () => {
  assert.deepEqual(
    tokenizeShellCommand("git rev-parse HEAD HEAD''^{tree}"),
    ["git", "rev-parse", "HEAD", "HEAD^{tree}"],
  );
});

test("reported commands match exact commands inside a shell wrapper", () => {
  const trace = [
    "/bin/sh -lc 'pwd && git -C primary rev-parse HEAD HEAD''^{tree} && git -C primary status --short'",
  ];
  assert.equal(
    reportedCommandMatchesTrace(
      "git -C primary rev-parse HEAD HEAD^{tree}",
      trace,
    ),
    true,
  );
  assert.deepEqual(commandTraceArgv(trace[0]), [
    ["pwd"],
    ["git", "-C", "primary", "rev-parse", "HEAD", "HEAD^{tree}"],
    ["git", "-C", "primary", "status", "--short"],
  ]);
});

test("multiline shell scripts expose each exact command", () => {
  const trace = [
    "/bin/sh -c 'git status --short\ngit rev-parse HEAD HEAD''^{tree}'",
  ];
  assert.equal(
    reportedCommandMatchesTrace("git rev-parse HEAD HEAD^{tree}", trace),
    true,
  );
});

test("comparison rejects omitted, reordered, aliased, and substring commands", () => {
  const trace = ["/bin/sh -c 'git -C primary rev-parse HEAD HEAD''^{tree}'"];
  assert.equal(
    reportedCommandMatchesTrace("git -C primary rev-parse HEAD", trace),
    false,
  );
  assert.equal(
    reportedCommandMatchesTrace(
      "git rev-parse -C primary HEAD HEAD^{tree}",
      trace,
    ),
    false,
  );
  assert.equal(
    reportedCommandMatchesTrace(
      "git -C ./primary rev-parse HEAD HEAD^{tree}",
      trace,
    ),
    false,
  );
  assert.equal(reportedCommandMatchesTrace("git -C primary", trace), false);
});

test("comparison rejects command substitution and compound reports", () => {
  const trace = ["/bin/sh -c 'git status --short && git rev-parse HEAD'"];
  assert.equal(reportedCommandMatchesTrace("echo $(git status)", trace), false);
  assert.equal(
    reportedCommandMatchesTrace(
      "git status --short && git rev-parse HEAD",
      trace,
    ),
    false,
  );
});
