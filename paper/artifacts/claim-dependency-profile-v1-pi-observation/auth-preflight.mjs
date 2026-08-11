#!/usr/bin/env node
import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  openSync,
  realpathSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, parse, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PROVIDER = "openai-codex";
export const MINIMUM_VALIDITY_MS = 21_600_000;
export const REFRESH_SENTINEL = "vela-nonrefreshable-sentinel-v1";
const MAXIMUM_AUTH_BYTES = 131_072;
const PROVIDER_ENVIRONMENT = [
  "OPENAI_API_KEY",
  "OPENAI_BASE_URL",
  "OPENAI_ORG_ID",
  "OPENAI_ORGANIZATION",
  "OPENAI_PROJECT",
  "OPENAI_PROJECT_ID",
  "HTTP_PROXY",
  "HTTPS_PROXY",
  "ALL_PROXY",
  "NO_PROXY",
  "http_proxy",
  "https_proxy",
  "all_proxy",
  "no_proxy",
];

export class AuthContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "AuthContractError";
  }
}

function requireContract(condition, message) {
  if (!condition) throw new AuthContractError(message);
}

function exactKeys(value, keys, label) {
  requireContract(value !== null && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  requireContract(JSON.stringify(actual) === JSON.stringify(expected), `${label} key set is invalid`);
}

function decodeBase64Url(value, label) {
  requireContract(typeof value === "string" && /^[A-Za-z0-9_-]+$/u.test(value), `${label} is not base64url`);
  try {
    return Buffer.from(value, "base64url").toString("utf8");
  } catch (error) {
    throw new AuthContractError(`${label} cannot be decoded: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function jwtPayload(token) {
  requireContract(typeof token === "string" && token.length <= 32_768, "access token is invalid");
  const parts = token.split(".");
  requireContract(parts.length === 3, "access token is not a JWT");
  let payload;
  try {
    payload = JSON.parse(decodeBase64Url(parts[1], "JWT payload"));
  } catch (error) {
    if (error instanceof AuthContractError) throw error;
    throw new AuthContractError(`JWT payload is not JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  requireContract(payload !== null && typeof payload === "object" && !Array.isArray(payload), "JWT payload must be an object");
  return payload;
}

function tokenBinding(access) {
  const payload = jwtPayload(access);
  const auth = payload["https://api.openai.com/auth"];
  requireContract(auth !== null && typeof auth === "object" && !Array.isArray(auth), "JWT OpenAI auth claim is missing");
  const accountId = auth.chatgpt_account_id;
  requireContract(typeof accountId === "string" && accountId.length > 0 && accountId.length <= 256, "JWT account ID is invalid");
  requireContract(Number.isSafeInteger(payload.exp) && payload.exp > 0, "JWT expiry is invalid");
  return { accountId, expires: payload.exp * 1000 };
}

function readRegular(path, { exactMode, privateMode = false } = {}) {
  const resolved = resolve(path);
  let beforePath;
  try {
    beforePath = lstatSync(resolved);
  } catch {
    throw new AuthContractError("auth file cannot be inspected");
  }
  requireContract(beforePath.isFile() && !beforePath.isSymbolicLink(), "auth path is not a regular file");
  let descriptor;
  try {
    descriptor = openSync(resolved, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  } catch {
    throw new AuthContractError("auth file cannot be opened");
  }
  try {
    try {
      const before = fstatSync(descriptor);
      const mode = before.mode & 0o777;
      requireContract(before.isFile() && before.size <= MAXIMUM_AUTH_BYTES, "auth file type or size is invalid");
      if (exactMode !== undefined) requireContract(mode === exactMode, `auth file mode must be ${exactMode.toString(8).padStart(4, "0")}`);
      if (privateMode) requireContract((mode & 0o077) === 0, "source auth file must not be group- or world-accessible");
      const data = readFileSync(descriptor);
      const after = fstatSync(descriptor);
      const afterPath = lstatSync(resolved);
      requireContract(
        before.dev === after.dev && before.ino === after.ino && before.size === after.size && before.mtimeMs === after.mtimeMs &&
          before.dev === afterPath.dev && before.ino === afterPath.ino && data.length === before.size,
        "auth file changed while reading",
      );
      return data;
    } catch (error) {
      if (error instanceof AuthContractError) throw error;
      throw new AuthContractError("auth file read or identity check failed");
    }
  } finally {
    try { closeSync(descriptor); } catch {}
  }
}

function assertPrivateOutputParent(output) {
  const parent = dirname(output);
  let metadata;
  try {
    metadata = lstatSync(parent);
  } catch {
    throw new AuthContractError("derived auth parent cannot be inspected");
  }
  requireContract(metadata.isDirectory() && !metadata.isSymbolicLink(), "derived auth parent must be a real directory");
  requireContract(metadata.uid === process.geteuid(), "derived auth parent must be owned by the current user");
  requireContract((metadata.mode & 0o077) === 0, "derived auth parent must not be group- or world-accessible");
  let canonicalParent;
  try {
    canonicalParent = realpathSync(parent);
  } catch {
    throw new AuthContractError("derived auth parent canonicalization failed");
  }
  requireContract(canonicalParent === parent, "derived auth parent must be canonical and symlink-free");
  let current = parent;
  const filesystemRoot = parse(current).root;
  while (true) {
    try {
      lstatSync(join(current, ".git"));
      throw new AuthContractError("derived auth must be outside every Git worktree and Git directory");
    } catch (error) {
      if (error instanceof AuthContractError) throw error;
      if (error?.code !== "ENOENT") throw new AuthContractError("Git-boundary inspection failed");
    }
    let head;
    let objects;
    try {
      head = lstatSync(join(current, "HEAD"));
      objects = lstatSync(join(current, "objects"));
    } catch (error) {
      if (error?.code !== "ENOENT") throw new AuthContractError("bare-Git boundary inspection failed");
    }
    requireContract(!(head?.isFile() && objects?.isDirectory()), "derived auth must be outside every bare Git directory");
    if (current === filesystemRoot) break;
    current = dirname(current);
  }
}

function parseJson(data, label) {
  let value;
  try {
    value = JSON.parse(data.toString("utf8"));
  } catch (error) {
    throw new AuthContractError(`${label} is not valid UTF-8 JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  return value;
}

function canonicalAuthBytes(value) {
  return Buffer.from(`${JSON.stringify(value)}\n`, "utf8");
}

export function validatePiAuthValue(value, { now = Date.now(), minimumValidityMs = MINIMUM_VALIDITY_MS } = {}) {
  exactKeys(value, [PROVIDER], "Pi auth");
  const credential = value[PROVIDER];
  exactKeys(credential, ["type", "access", "refresh", "expires", "accountId"], "Pi OAuth credential");
  requireContract(credential.type === "oauth", "Pi credential type must be oauth");
  requireContract(typeof credential.access === "string" && credential.access.length > 0, "Pi access token is missing");
  requireContract(credential.refresh === REFRESH_SENTINEL, "Pi credential must use the fixed non-refreshable sentinel");
  requireContract(Number.isSafeInteger(credential.expires), "Pi credential expiry is invalid");
  requireContract(typeof credential.accountId === "string", "Pi credential account ID is invalid");
  const binding = tokenBinding(credential.access);
  requireContract(binding.accountId === credential.accountId, "Pi credential account ID does not match access JWT");
  requireContract(binding.expires === credential.expires, "Pi credential expiry does not match access JWT");
  requireContract(credential.expires - now >= minimumValidityMs, "Pi OAuth credential has insufficient remaining validity; refresh is forbidden");
  return Object.freeze({ ...credential });
}

export function loadFrozenOAuth(path, options = {}) {
  const bytes = readRegular(path, { exactMode: 0o400 });
  const value = parseJson(bytes, "Pi auth file");
  requireContract(bytes.equals(canonicalAuthBytes(value)), "Pi auth file must use the exact compact derived encoding");
  return validatePiAuthValue(value, options);
}

export function createReadOnlyCredentialStore(credential) {
  const frozen = Object.freeze({ ...credential });
  return Object.freeze({
    async read(providerId) {
      return providerId === PROVIDER ? { ...frozen } : undefined;
    },
    async list() {
      return Object.freeze([{ providerId: PROVIDER, type: "oauth" }]);
    },
    async modify() {
      throw new AuthContractError("OAuth credential mutation or refresh is forbidden");
    },
    async delete() {
      throw new AuthContractError("OAuth credential deletion is forbidden");
    },
  });
}

export function sanitizeProviderEnvironment(environment = process.env) {
  const removed = [];
  for (const name of PROVIDER_ENVIRONMENT) {
    if (Object.hasOwn(environment, name)) {
      removed.push(name);
      delete environment[name];
    }
  }
  return Object.freeze(removed.sort());
}

export function deriveFromCodexAuth(sourcePath, outputPath, { now = Date.now(), minimumValidityMs = MINIMUM_VALIDITY_MS } = {}) {
  const sourceBytes = readRegular(sourcePath, { privateMode: true });
  const source = parseJson(sourceBytes, "Codex auth file");
  requireContract(source !== null && typeof source === "object" && !Array.isArray(source), "Codex auth must be an object");
  const tokens = source.tokens;
  requireContract(tokens !== null && typeof tokens === "object" && !Array.isArray(tokens), "Codex token set is missing");
  const binding = tokenBinding(tokens.access_token);
  if (tokens.account_id !== undefined) requireContract(tokens.account_id === binding.accountId, "Codex account ID does not match access JWT");
  const value = {
    [PROVIDER]: {
      type: "oauth",
      access: tokens.access_token,
      refresh: REFRESH_SENTINEL,
      expires: binding.expires,
      accountId: binding.accountId,
    },
  };
  validatePiAuthValue(value, { now, minimumValidityMs });
  const output = resolve(outputPath);
  requireContract(resolve(dirname(output)) !== output, "derived auth output path is invalid");
  assertPrivateOutputParent(output);
  const bytes = canonicalAuthBytes(value);
  let descriptor;
  let created = false;
  let completed = false;
  try {
    descriptor = openSync(output, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | (constants.O_NOFOLLOW ?? 0), 0o400);
    created = true;
    writeFileSync(descriptor, bytes);
    fsyncSync(descriptor);
    fchmodSync(descriptor, 0o400);
    closeSync(descriptor);
    descriptor = undefined;
    loadFrozenOAuth(output, { now, minimumValidityMs });
    completed = true;
  } catch {
    throw new AuthContractError("derived auth creation or validation failed");
  } finally {
    if (descriptor !== undefined) {
      try { closeSync(descriptor); } catch {}
    }
    if (created && !completed) {
      try { unlinkSync(output); } catch {}
    }
  }
  return Object.freeze({
    provider: PROVIDER,
    credential_type: "oauth",
    validity_window: minimumValidityMs === MINIMUM_VALIDITY_MS ? "at_least_6h" : "minimum_satisfied",
    output_mode_0400: true,
    refresh_forbidden: true,
    real_refresh_copied: false,
    mutation_refused: true,
  });
}

function argumentsMap(argv) {
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    requireContract(argv[index]?.startsWith("--") && argv[index + 1] !== undefined, "arguments must be --name value pairs");
    requireContract(!result.has(argv[index]), `duplicate argument: ${argv[index]}`);
    result.set(argv[index], argv[index + 1]);
  }
  return result;
}

function main(argv) {
  const [command, ...rest] = argv;
  const args = argumentsMap(rest);
  if (command === "derive") {
    requireContract(args.size === 2 && args.has("--codex-auth") && args.has("--output"), "derive requires --codex-auth and --output");
    process.stdout.write(`${JSON.stringify(deriveFromCodexAuth(args.get("--codex-auth"), args.get("--output")))}\n`);
    return;
  }
  if (command === "check") {
    requireContract(args.size === 1 && args.has("--auth"), "check requires --auth");
    const credential = loadFrozenOAuth(args.get("--auth"));
    process.stdout.write(`${JSON.stringify({ provider: PROVIDER, credential_type: credential.type, validity_window: "at_least_6h", mode_0400: true, refresh_forbidden: true, real_refresh_copied: false, mutation_refused: true })}\n`);
    return;
  }
  throw new AuthContractError("usage: auth-preflight.mjs derive --codex-auth PATH --output PATH | check --auth PATH");
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`auth-preflight: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
