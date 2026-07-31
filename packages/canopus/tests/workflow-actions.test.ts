import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const workflows = new URL("../../../../.github/workflows/", import.meta.url);
const expectedNode24Pins = new Map([
  ["actions/checkout", "9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"],
  ["actions/setup-node", "820762786026740c76f36085b0efc47a31fe5020"],
  ["oven-sh/setup-bun", "0c5077e51419868618aeaa5fe8019c62421857d6"],
]);

test("workflow actions are immutable and Node tooling uses maintained runtimes", async () => {
  const files = (await readdir(workflows)).filter((file) => file.endsWith(".yml")).sort();
  assert.ok(
    !files.includes("verifier-images.yml"),
    "domain verifier images must be owned by their Frontier, not Vela Agent CI",
  );
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

test("one Vela tag owns public Protocol publication while Canopus stays private", async () => {
  const value = await readFile(new URL("release.yml", workflows), "utf8");
  const protocolStart = value.indexOf("  publish-protocol:\n");
  const registryStart = value.indexOf("  registry-smoke:\n");
  const releaseStart = value.indexOf("  publish:\n");
  assert.ok(protocolStart >= 0, "Protocol publication job is missing");
  assert.ok(registryStart > protocolStart, "Protocol publication must precede registry smoke");
  assert.ok(releaseStart > registryStart, "GitHub release must follow registry checks");

  const protocol = value.slice(protocolStart, registryStart);
  const release = value.slice(releaseStart);
  assert.match(protocol, /environment: npm/u);
  assert.match(protocol, /Smoke the exact packed Protocol package/u);
  assert.match(protocol, /npm install[\s\S]+--ignore-scripts/u);
  assert.match(protocol, /protocolDigest/u);
  assert.match(protocol, /sha256At/u);
  assert.match(protocol, /publish-protocol\.mjs check/u);
  assert.match(protocol, /publish-protocol\.mjs --execute/u);
  assert.match(protocol, /bun run --cwd packages\/protocol test/u);
  assert.doesNotMatch(protocol, /packages\/canopus/u);
  assert.match(release, /publish-protocol/u);
  assert.doesNotMatch(value, /product-v/u);
});
