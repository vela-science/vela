import path from "node:path";

export class ContractError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "ContractError";
  }
}

export type JsonObject = Record<string, unknown>;

export function objectAt(value: unknown, at: string): JsonObject {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ContractError(`${at} must be an object`);
  }
  return value as JsonObject;
}

export function exactKeys(
  value: JsonObject,
  required: readonly string[],
  optional: readonly string[],
  at: string,
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) {
      throw new ContractError(`${at}.${key} is not allowed`);
    }
  }
  for (const key of required) {
    if (!(key in value)) {
      throw new ContractError(`${at}.${key} is required`);
    }
  }
}

export function stringAt(
  value: unknown,
  at: string,
  options: { min?: number; max: number; pattern?: RegExp },
): string {
  if (typeof value !== "string") {
    throw new ContractError(`${at} must be a string`);
  }
  const min = options.min ?? 0;
  if (value.length < min || value.length > options.max) {
    throw new ContractError(`${at} length must be ${min}..${options.max}`);
  }
  if (options.pattern !== undefined && !options.pattern.test(value)) {
    throw new ContractError(`${at} has an invalid format`);
  }
  return value;
}

export function integerAt(
  value: unknown,
  at: string,
  min: number,
  max: number,
): number {
  if (!Number.isInteger(value) || typeof value !== "number") {
    throw new ContractError(`${at} must be an integer`);
  }
  if (value < min || value > max) {
    throw new ContractError(`${at} must be ${min}..${max}`);
  }
  return value;
}

export function enumAt<const T extends string>(
  value: unknown,
  at: string,
  allowed: readonly T[],
): T {
  if (typeof value !== "string" || !allowed.includes(value as T)) {
    throw new ContractError(`${at} must be one of ${allowed.join(", ")}`);
  }
  return value as T;
}

export function arrayAt<T>(
  value: unknown,
  at: string,
  options: { min?: number; max: number; unique?: boolean },
  parse: (item: unknown, at: string) => T,
): T[] {
  if (!Array.isArray(value)) {
    throw new ContractError(`${at} must be an array`);
  }
  const min = options.min ?? 0;
  if (value.length < min || value.length > options.max) {
    throw new ContractError(`${at} length must be ${min}..${options.max}`);
  }
  const parsed = value.map((item, index) => parse(item, `${at}[${index}]`));
  if (options.unique === true) {
    const seen = new Set(parsed.map((item) => JSON.stringify(item)));
    if (seen.size !== parsed.length) {
      throw new ContractError(`${at} must not contain duplicates`);
    }
  }
  return parsed;
}

export const GIT_OBJECT_RE = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u;
export const SHA256_RE = /^sha256:[0-9a-f]{64}$/u;
export const MISSION_ID_RE = /^mission_[a-z0-9][a-z0-9._-]{0,126}$/u;
export const AGENT_RE = /^agent:[a-z0-9][a-z0-9._-]{0,62}$/u;

export function relativePathAt(value: unknown, at: string): string {
  const parsed = stringAt(value, at, { min: 1, max: 1024 });
  if (parsed.includes("\0") || path.isAbsolute(parsed) || parsed.includes("\\")) {
    throw new ContractError(`${at} must be a portable relative path`);
  }
  const normalized = path.posix.normalize(parsed);
  if (
    normalized === "." ||
    normalized === ".." ||
    normalized.startsWith("../") ||
    normalized !== parsed
  ) {
    throw new ContractError(`${at} must be normalized and remain below its root`);
  }
  return parsed;
}

export function sha256At(value: unknown, at: string): string {
  return stringAt(value, at, { max: 71, pattern: SHA256_RE });
}

export function gitObjectAt(value: unknown, at: string): string {
  return stringAt(value, at, { max: 64, pattern: GIT_OBJECT_RE });
}

