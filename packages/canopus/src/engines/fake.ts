import { contentDigest, sha256Bytes } from "../util/canonical.js";
import type { CandidateDraft, Engine, EngineContext, EngineResult } from "./engine.js";

export class FakeEngine implements Engine {
  public readonly name = "fake";
  readonly #draft: CandidateDraft | ((context: EngineContext) => CandidateDraft | Promise<CandidateDraft>);

  public constructor(
    draft: CandidateDraft | ((context: EngineContext) => CandidateDraft | Promise<CandidateDraft>),
  ) {
    this.#draft = draft;
  }

  public async run(context: EngineContext): Promise<EngineResult> {
    const started = performance.now();
    context.budget.beginAttempt();
    context.budget.beginProcess();
    const draft =
      typeof this.#draft === "function" ? await this.#draft(context) : structuredClone(this.#draft);
    const serialized = JSON.stringify(draft);
    context.budget.addOutput(Buffer.byteLength(serialized));
    return {
      draft,
      engine: {
        name: this.name,
        version: "1",
        binary_sha256: null,
        model: null,
        configuration_sha256: contentDigest({ engine: this.name, version: "1" }),
      },
      usage: {
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
      },
      wallTimeMs: Math.max(0, Math.round(performance.now() - started)),
      eventTypes: ["fake.completed"],
      actionTypes: [],
      eventsDigest: contentDigest({ type: "fake.completed", output: draft }),
      stderrDigest: sha256Bytes(""),
    };
  }
}
