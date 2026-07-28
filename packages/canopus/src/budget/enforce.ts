import type { MissionBudgets } from "../contracts/mission.js";

export type BudgetDimension =
  | "max_research_wall_time_ms"
  | "max_research_processes"
  | "max_research_output_bytes"
  | "max_prompt_bytes"
  | "max_artifact_bytes"
  | "max_attempts"
  | "max_observed_tokens";

export class BudgetExceeded extends Error {
  public readonly dimension: BudgetDimension;

  public constructor(dimension: BudgetDimension, used: number, limit: number) {
    super(`${dimension} budget exceeded: ${used} > ${limit}`);
    this.name = "BudgetExceeded";
    this.dimension = dimension;
  }
}

export interface BudgetSnapshot {
  research_elapsed_ms: number;
  research_processes: number;
  research_output_bytes: number;
  prompt_bytes: number;
  artifact_bytes: number;
  attempts: number;
  observed_tokens: number;
}

export class BudgetTracker {
  readonly #limits: MissionBudgets;
  readonly #now: () => number;
  readonly #started: number;
  #processes = 0;
  #outputBytes = 0;
  #promptBytes = 0;
  #artifactBytes = 0;
  #attempts = 0;
  #tokens = 0;

  public constructor(limits: MissionBudgets, now: () => number = performance.now.bind(performance)) {
    this.#limits = { ...limits };
    this.#now = now;
    this.#started = now();
  }

  #assert(dimension: BudgetDimension, used: number, limit: number): void {
    if (used > limit) {
      throw new BudgetExceeded(dimension, used, limit);
    }
  }

  public assertTime(): number {
    const elapsed = Math.max(0, Math.round(this.#now() - this.#started));
    this.#assert(
      "max_research_wall_time_ms",
      elapsed,
      this.#limits.max_research_wall_time_ms,
    );
    return elapsed;
  }

  public beginAttempt(): number {
    this.assertTime();
    const next = this.#attempts + 1;
    this.#assert("max_attempts", next, this.#limits.max_attempts);
    this.#attempts = next;
    return next;
  }

  public beginProcess(): number {
    this.assertTime();
    const next = this.#processes + 1;
    this.#assert(
      "max_research_processes",
      next,
      this.#limits.max_research_processes,
    );
    this.#processes = next;
    return next;
  }

  public addOutput(bytes: number): number {
    this.assertTime();
    const next = this.#outputBytes + bytes;
    this.#assert(
      "max_research_output_bytes",
      next,
      this.#limits.max_research_output_bytes,
    );
    this.#outputBytes = next;
    return next;
  }

  public addPrompt(bytes: number): number {
    this.assertTime();
    const next = this.#promptBytes + bytes;
    this.#assert("max_prompt_bytes", next, this.#limits.max_prompt_bytes);
    this.#promptBytes = next;
    return next;
  }

  public addArtifact(bytes: number): number {
    this.assertTime();
    const next = this.#artifactBytes + bytes;
    this.#assert("max_artifact_bytes", next, this.#limits.max_artifact_bytes);
    this.#artifactBytes = next;
    return next;
  }

  public addTokens(tokens: number): number {
    this.assertTime();
    const next = this.#tokens + tokens;
    this.#assert("max_observed_tokens", next, this.#limits.max_observed_tokens);
    this.#tokens = next;
    return next;
  }

  public remainingTimeMs(): number {
    return Math.max(0, this.#limits.max_research_wall_time_ms - this.assertTime());
  }

  public remainingOutputBytes(): number {
    this.assertTime();
    return Math.max(0, this.#limits.max_research_output_bytes - this.#outputBytes);
  }

  public remainingArtifactBytes(): number {
    this.assertTime();
    return Math.max(0, this.#limits.max_artifact_bytes - this.#artifactBytes);
  }

  public snapshot(): BudgetSnapshot {
    return {
      research_elapsed_ms: this.assertTime(),
      research_processes: this.#processes,
      research_output_bytes: this.#outputBytes,
      prompt_bytes: this.#promptBytes,
      artifact_bytes: this.#artifactBytes,
      attempts: this.#attempts,
      observed_tokens: this.#tokens,
    };
  }
}
