export function assertToolUsingMissionPlatform(
  platform: NodeJS.Platform = process.platform,
): void {
  if (platform === "win32") {
    throw new Error(
      "tool-using missions require WSL2 on Windows; enter the frontier through its Linux path and rerun the same vela agent command",
    );
  }
  if (platform !== "darwin" && platform !== "linux") {
    throw new Error(
      `tool-using missions are unsupported on ${platform}; supported worker hosts are macOS and Linux/WSL2`,
    );
  }
}
