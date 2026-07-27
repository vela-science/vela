import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const workflows = new URL("../../.github/workflows/", import.meta.url);
const expectedNode24Pins = new Map([
  ["actions/checkout", "9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"],
  ["actions/setup-node", "820762786026740c76f36085b0efc47a31fe5020"],
  ["oven-sh/setup-bun", "0c5077e51419868618aeaa5fe8019c62421857d6"],
  ["docker/login-action", "af1e73f918a031802d376d3c8bbc3fe56130a9b0"],
  ["docker/setup-buildx-action", "bb05f3f5519dd87d3ba754cc423b652a5edd6d2c"],
  ["docker/build-push-action", "53b7df96c91f9c12dcc8a07bcb9ccacbed38856a"],
]);

test("workflow actions are immutable and Node tooling uses maintained runtimes", async () => {
  const files = (await readdir(workflows)).filter((file) => file.endsWith(".yml")).sort();
  const observedNodePins = new Map<string, Set<string>>();
  for (const file of files) {
    const value = await readFile(new URL(file, workflows), "utf8");
    for (const match of value.matchAll(/^\s*-?\s*uses:\s*([^\s@]+)@([^\s#]+)/gmu)) {
      const action = match[1] as string;
      const pin = match[2] as string;
      assert.match(pin, /^[0-9a-f]{40}$/u, `${file}: ${action} must use a commit SHA`);
      if (expectedNode24Pins.has(action)) {
        const pins = observedNodePins.get(action) ?? new Set<string>();
        pins.add(pin);
        observedNodePins.set(action, pins);
      }
    }
  }
  for (const [action, expected] of expectedNode24Pins) {
    assert.deepEqual(observedNodePins.get(action), new Set([expected]), `${action} pin drifted`);
  }
});

test("release validates the complete macOS product before the portable OIDC publisher", async () => {
  const value = await readFile(new URL("release.yml", workflows), "utf8");
  const validateStart = value.indexOf("  validate:\n");
  const publishStart = value.indexOf("  publish:\n");
  assert.ok(validateStart >= 0, "release validate job is missing");
  assert.ok(publishStart > validateStart, "release publish job must follow validation");

  const validate = value.slice(validateStart, publishStart);
  const publish = value.slice(publishStart);
  assert.match(validate, /runs-on: macos-15/u);
  assert.match(validate, /- run: bun run check/u);
  assert.match(publish, /needs: validate/u);
  assert.doesNotMatch(publish, /- run: bun run check/u);
  assert.match(publish, /bun run typecheck/u);
  assert.match(publish, /dist\/tests\/release-contract\.test\.js/u);
});
