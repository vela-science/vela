import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import process from "node:process";

export type CommandFailureKind =
  | "invalid_argv"
  | "spawn"
  | "timeout"
  | "output_limit"
  | "aborted";

export class CommandFailure extends Error {
  public readonly kind: CommandFailureKind;

  public constructor(kind: CommandFailureKind, message: string) {
    super(message);
    this.name = "CommandFailure";
    this.kind = kind;
  }
}

export interface CommandResult {
  argv: string[];
  exitCode: number;
  signal: NodeJS.Signals | null;
  stdout: Buffer;
  stderr: Buffer;
  durationMs: number;
}

export interface CommandOptions {
  argv: readonly string[];
  cwd: string;
  env: NodeJS.ProcessEnv;
  timeoutMs: number;
  maxOutputBytes: number;
  stdin?: Uint8Array | string;
  signal?: AbortSignal;
}

export type CommandRunner = (options: CommandOptions) => Promise<CommandResult>;
export const MAX_COMMAND_ARGV = 128;

function validateArgv(argv: readonly string[]): void {
  if (argv.length === 0 || argv.length > MAX_COMMAND_ARGV) {
    throw new CommandFailure(
      "invalid_argv",
      `argv must contain 1..${MAX_COMMAND_ARGV} entries`,
    );
  }
  for (const [index, item] of argv.entries()) {
    if (item.length === 0 || item.length > 16_384 || item.includes("\0")) {
      throw new CommandFailure("invalid_argv", `argv[${index}] is invalid`);
    }
  }
}

function terminateTree(pid: number | undefined, signal: NodeJS.Signals): void {
  if (pid === undefined) {
    return;
  }
  try {
    if (process.platform === "win32") {
      process.kill(pid, signal);
    } else {
      process.kill(-pid, signal);
    }
  } catch {
    // The process may already have exited.
  }
}

export function isolatedEnvironment(
  home: string,
  platform: NodeJS.Platform = process.platform,
): NodeJS.ProcessEnv {
  const path = process.env.PATH;
  const common: NodeJS.ProcessEnv = {
    ...(path === undefined ? {} : { PATH: path }),
    HOME: home,
    XDG_CONFIG_HOME: `${home}/.config`,
    XDG_CACHE_HOME: `${home}/.cache`,
    XDG_DATA_HOME: `${home}/.local/share`,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_TERMINAL_PROMPT: "0",
    VELA_NO_KEY_ACCESS: "1",
    NO_PROXY: "*",
    no_proxy: "*",
  };
  if (platform === "win32") {
    return {
      ...common,
      USERPROFILE: home,
      APPDATA: `${home}\\AppData\\Roaming`,
      LOCALAPPDATA: `${home}\\AppData\\Local`,
      GIT_CONFIG_GLOBAL: "NUL",
    };
  }
  return {
    ...common,
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    // rustls-native-certs checks this before macOS keychains. Pinning the
    // system PEM bundle keeps TLS functional without exposing user keychains.
    SSL_CERT_FILE: "/etc/ssl/cert.pem",
    GIT_CONFIG_GLOBAL: "/dev/null",
  };
}

export async function runCommand(options: CommandOptions): Promise<CommandResult> {
  validateArgv(options.argv);
  if (!Number.isInteger(options.timeoutMs) || options.timeoutMs < 1) {
    throw new CommandFailure("invalid_argv", "timeoutMs must be a positive integer");
  }
  if (!Number.isInteger(options.maxOutputBytes) || options.maxOutputBytes < 1) {
    throw new CommandFailure("invalid_argv", "maxOutputBytes must be a positive integer");
  }
  if (options.signal?.aborted === true) {
    throw new CommandFailure("aborted", "command was aborted before spawn");
  }
  const executable = options.argv[0];
  if (executable === undefined) {
    throw new CommandFailure("invalid_argv", "argv must name an executable");
  }

  const started = performance.now();
  return await new Promise<CommandResult>((resolve, reject) => {
    const child: ChildProcessWithoutNullStreams = spawn(executable, options.argv.slice(1), {
      cwd: options.cwd,
      env: options.env,
      detached: process.platform !== "win32",
      shell: false,
      stdio: "pipe",
      windowsHide: true,
    });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    let bytes = 0;
    let failure: CommandFailure | undefined;
    let killTimer: NodeJS.Timeout | undefined;

    const terminate = (nextFailure: CommandFailure): void => {
      if (failure !== undefined) {
        return;
      }
      failure = nextFailure;
      terminateTree(child.pid, "SIGTERM");
      killTimer = setTimeout(() => terminateTree(child.pid, "SIGKILL"), 250);
      killTimer.unref();
    };

    const collect = (target: Buffer[], chunk: Buffer): void => {
      bytes += chunk.length;
      if (bytes > options.maxOutputBytes) {
        terminate(
          new CommandFailure(
            "output_limit",
            `command output exceeded ${options.maxOutputBytes} bytes`,
          ),
        );
        return;
      }
      target.push(chunk);
    };

    child.stdout.on("data", (chunk: Buffer) => collect(stdout, chunk));
    child.stderr.on("data", (chunk: Buffer) => collect(stderr, chunk));

    const timeout = setTimeout(
      () =>
        terminate(
          new CommandFailure("timeout", `command exceeded ${options.timeoutMs} milliseconds`),
        ),
      options.timeoutMs,
    );
    timeout.unref();

    const abort = (): void => terminate(new CommandFailure("aborted", "command was aborted"));
    options.signal?.addEventListener("abort", abort, { once: true });

    child.once("error", (error) => {
      clearTimeout(timeout);
      if (killTimer !== undefined) clearTimeout(killTimer);
      options.signal?.removeEventListener("abort", abort);
      reject(new CommandFailure("spawn", `could not spawn ${executable}: ${error.message}`));
    });

    child.once("close", (code, signal) => {
      clearTimeout(timeout);
      if (killTimer !== undefined) clearTimeout(killTimer);
      options.signal?.removeEventListener("abort", abort);
      // A verifier may fork, close inherited stdio, and let its parent exit.
      // The wrapper's successful exit is therefore not sufficient: terminate
      // the entire detached process group before returning in every outcome.
      terminateTree(child.pid, "SIGTERM");
      terminateTree(child.pid, "SIGKILL");
      if (failure !== undefined) {
        reject(failure);
        return;
      }
      resolve({
        argv: [...options.argv],
        exitCode: code ?? -1,
        signal,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
        durationMs: Math.round(performance.now() - started),
      });
    });

    if (options.stdin === undefined) {
      child.stdin.end();
    } else {
      child.stdin.end(options.stdin);
    }
  });
}
