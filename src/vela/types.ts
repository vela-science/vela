import type { MissionRoots } from "../contracts/mission.js";

export interface VelaInspection {
  version: string;
  roots: MissionRoots;
  status: Record<string, unknown>;
  repository: Record<string, unknown>;
}

export interface VelaCommandResponse {
  ok: true;
  value: Record<string, unknown>;
}
