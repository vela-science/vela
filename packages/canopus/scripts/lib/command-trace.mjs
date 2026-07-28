import path from "node:path";

const separators = new Set(["&&", "||", ";", "|", "\n"]);

function isWhitespace(character) {
  return character === " " || character === "\t" || character === "\r";
}

export function tokenizeShellCommand(input) {
  if (typeof input !== "string") {
    throw new TypeError("shell command must be a string");
  }
  const tokens = [];
  let current = "";
  let started = false;
  let state = "normal";

  const flush = () => {
    if (!started) return;
    tokens.push(current);
    current = "";
    started = false;
  };

  for (let index = 0; index < input.length; index += 1) {
    const character = input[index];
    const next = input[index + 1];

    if (state === "single") {
      if (character === "'") {
        state = "normal";
      } else {
        current += character;
      }
      continue;
    }

    if (state === "double") {
      if (character === '"') {
        state = "normal";
      } else if (character === "\\") {
        if (next === undefined) throw new Error("unterminated shell escape");
        current += next;
        index += 1;
      } else if (character === "`" || (character === "$" && next === "(")) {
        throw new Error("command substitution is not a comparable command trace");
      } else {
        current += character;
      }
      continue;
    }

    if (character === "`" || (character === "$" && next === "(")) {
      throw new Error("command substitution is not a comparable command trace");
    }
    if (character === "'") {
      state = "single";
      started = true;
      continue;
    }
    if (character === '"') {
      state = "double";
      started = true;
      continue;
    }
    if (character === "\\") {
      if (next === undefined) throw new Error("unterminated shell escape");
      current += next;
      started = true;
      index += 1;
      continue;
    }
    if (character === "\n") {
      flush();
      tokens.push("\n");
      continue;
    }
    if (isWhitespace(character)) {
      flush();
      continue;
    }
    if ((character === "&" && next === "&") || (character === "|" && next === "|")) {
      flush();
      tokens.push(`${character}${next}`);
      index += 1;
      continue;
    }
    if (character === ";" || character === "|") {
      flush();
      tokens.push(character);
      continue;
    }
    current += character;
    started = true;
  }

  if (state !== "normal") throw new Error("unterminated shell quote");
  flush();
  return tokens;
}

function splitSimpleCommands(tokens) {
  const commands = [];
  let current = [];
  for (const token of tokens) {
    if (separators.has(token)) {
      if (current.length > 0) commands.push(current);
      current = [];
    } else {
      current.push(token);
    }
  }
  if (current.length > 0) commands.push(current);
  return commands;
}

function shellScriptArgument(tokens) {
  if (tokens.length < 3) return null;
  const shell = path.basename(tokens[0]);
  if (!["sh", "bash", "zsh"].includes(shell)) return null;
  for (let index = 1; index < tokens.length - 1; index += 1) {
    const option = tokens[index];
    if (/^-[a-zA-Z]*c[a-zA-Z]*$/u.test(option)) return tokens[index + 1];
  }
  return null;
}

export function commandTraceArgv(command) {
  const tokens = tokenizeShellCommand(command);
  const script = shellScriptArgument(tokens);
  if (script !== null) {
    return splitSimpleCommands(tokenizeShellCommand(script));
  }
  return splitSimpleCommands(tokens);
}

export function reportedCommandMatchesTrace(reported, tracedCommands) {
  let reportedCommands;
  try {
    reportedCommands = commandTraceArgv(reported);
  } catch {
    return false;
  }
  if (reportedCommands.length !== 1) return false;
  const expected = JSON.stringify(reportedCommands[0]);
  for (const traced of tracedCommands) {
    let candidates;
    try {
      candidates = commandTraceArgv(traced);
    } catch {
      continue;
    }
    if (candidates.some((candidate) => JSON.stringify(candidate) === expected)) {
      return true;
    }
  }
  return false;
}
