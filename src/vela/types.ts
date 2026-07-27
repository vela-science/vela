import type { MissionRoots } from "../contracts/mission.js";

export interface VelaInspection {
  version: string;
  roots: MissionRoots;
  check: Record<string, unknown>;
  proof: Record<string, unknown>;
}

export interface VelaCommandResponse {
  ok: true;
  value: Record<string, unknown>;
}
