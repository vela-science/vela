# Science Needs State

Two papers came out the same week. One reported that a humanized antibody crossed the blood-brain barrier in mice when conjugated to a transferrin-receptor binder. The other reported it didn't. They contradicted each other on the headline result, the dose-response shape, and the proposed mechanism.

A foundation officer read both. She had forty million dollars and a five-year window for an Alzheimer's translation program, and the question of whether you could deliver therapeutic antibodies to brain parenchyma at all was load-bearing for the entire portfolio. Where did the contradiction get resolved?

Not in either paper. Each was internally consistent. Not in the journal. Neither editor was going to publish a meta-correction. Not in the next conference, where both labs would present their data confidently and not look at each other. Not in any lab notebook, because lab notebooks belong to one person and don't compose. The contradiction got resolved in her head, in the shower, the next morning, when she remembered something a third lab had told her in confidence about the assay conditions. She made a decision. The decision was right. The reasoning evaporated.

This is everywhere in science, and it is the default. Scientific knowledge, in 2026, is held in three substrates: prose, headlines, and people. None of these substrates compose. None of them replay. None of them survive the death or distraction of a particular reviewer. The contradictions are real, they get resolved by individuals on a case-by-case basis, and the resolution evaporates.

The thing science is missing is *state*.

---

By state I mean what software engineers mean. Not the state of the field; that's a metaphor. I mean an artifact you can name, fork, diff, and replay. Something that has identity over time. Something whose contents don't drift when you stop looking at it.

Software has this. Git has it. A repository at a particular commit is unambiguous: anyone with the SHA can reconstruct exactly what was there. Two engineers thousands of miles apart can argue about a function and resolve the argument by pointing at a file at a line at a commit. The substrate carries the argument. The substrate is what makes engineering compositional in a way that science still isn't.

Science has tried to get this property in a few different ways and mostly failed.

Preprints help with timing but not with state. A preprint is a snapshot of prose. If two preprints contradict each other, the contradiction lives in the reader's head. Bibliographic databases (PubMed, Crossref, OpenAlex) catalog the prose. They do not encode what any paper claims, only that the paper exists. Citation graphs encode that A pointed at B, not whether A agrees with B or replicated B or extended B or refuted B.

Lab notebooks help with reproducibility within one lab and almost not at all between labs. The notebook is private, its conventions are private, and the handful of attempts to standardize it (ELNs, Notion-for-science, structured data deposits) keep running into the same ceiling: a researcher who is graded on papers will not maintain a parallel artifact unless the parallel artifact gets them a paper.

Knowledge graphs were the most ambitious attempt. The vision was right. If you could encode every claim in every paper as a triple, you could query the entire literature like a database. The execution was impossible. Building the graph required either heroic curators (didn't scale) or NLP that was good enough to extract claims faithfully (didn't exist). The graphs that got built were brittle, and the maintenance cost compounded. They went stale.

Two things have changed since the knowledge-graph wave broke.

The first is that language models are now genuinely good at structured extraction. Not perfect. They hallucinate, they smooth over caveats, they confuse what a paper claims with what it cites. But with the right prompting and the right scaffolding, a model can read a paper and produce a structured claim object that's right more often than it's wrong. The expensive curatorial step that broke the previous knowledge graphs is no longer expensive.

The second is that we now know what a structured claim object should look like. The mistakes of the 2010s are documented. A claim is not a triple. A claim has a confidence, a scope (the population, the model organism, the dose, the assay), evidence, conditions, and provenance. A claim should be content-addressed, so two implementations can agree it's the same claim without trusting each other. A claim should have an event log, so when someone disputes it, the dispute becomes part of the artifact rather than a comment that goes unread.

You can build that substrate now. The question is what the minimum viable shape is.

---

Here's the minimum viable shape.

Start with a *finding bundle*. A finding bundle is a typed object that represents one assertion: what was observed, in what conditions, with what evidence, with what confidence, by whom, citing what. It has a content-addressed identifier, a SHA-256 of its canonical-JSON representation, so two implementations of the protocol agree byte-for-byte on whether two findings are the same finding.

Findings live in a *frontier*. A frontier is a bounded scope: "what we currently believe about the blood-brain barrier in Alzheimer's translation." A frontier holds findings, the links between them, the actors who can write into it, and the event log that records every change. The frontier itself has an identifier, a content-addressed `vfr_id`. Two reviewers in different timezones can reference the same frontier the same way two engineers reference the same git repository.

Changes enter the frontier as *proposals*. A proposal is "I think this finding should be revised, retracted, contested, or annotated, and here is my reasoning." Proposals are signed by the proposer's Ed25519 key. They become canonical events when reviewed and accepted. The event log is the history. Replay the events on a fresh frontier and you reconstruct the current state byte-for-byte. This is how git stays honest, and it's how a scientific frontier can stay honest too.

Anyone can publish a frontier to a *registry*. The registry is the discovery layer. A registry entry is a signed manifest: here is the `vfr_id` I produced, here is its current snapshot hash, here is its current event-log hash, here is the locator where you can fetch the actual frontier file. A reader pulls, verifies the signature, verifies the hashes, and now has a byte-identical reconstruction of what the publisher saw.

That's the substrate. Five primitives: finding, frontier, proposal, event, registry entry. Everything else (search, agents, dashboards, peer review tooling) is built on top.

---

This substrate now exists.

The reference implementation is Vela. It is a single Rust binary, one Postgres-backed hub, a Python SDK, an MCP server, and a small browser-based review surface called the Workbench. It has no proprietary dependencies. The protocol is the contract; any second implementation that follows the canonical-JSON discipline produces byte-identical IDs and signatures.

There is one frontier you can pull right now. It's called BBB Flagship. It contains forty-eight signed findings about the blood-brain barrier and Alzheimer's translation: the antibody-conjugate work, the transferrin-receptor mechanism, the contradictory delivery results, the assay-condition reasoning that resolves them. It has a `vfr_id` that re-derives from canonical JSON alone, a snapshot hash that any client can verify, and an event log that replays deterministically.

```
vela registry pull vfr_7344e96c0f2669d5 \
  --from https://vela-hub.fly.dev/entries \
  --out ./bbb.json
```

The frontier file is JSON. Open it in a text editor; every claim is human-readable. Pass it to the Python SDK; it's queryable. Open it in the Workbench; every finding has a triage view with the evidence, the proposals against it, and the event history.

The hub at <https://vela-hub.fly.dev> re-publishes the BBB frontier every Monday at 14:00 UTC, signed by a CI bot whose public key is registered in the frontier itself. Anyone with an Ed25519 key can publish their own `vfr_id` to the same hub. The signature is the bind. There is no allowlist. There is no operator privilege.

This is a small thing. It is also the smallest thing that is actually a substrate.

---

Once this exists, things compose.

A domain expert who disagrees with a finding can publish a proposal signed under their own actor identity, and the proposal carries the same provenance metadata the finding does. The dispute becomes part of the artifact rather than a remark in a coffee break. An agent reading the frontier can route proposals to the right reviewer based on the actor registry. A foundation officer with forty million dollars can pull a frontier, see the contradictions explicitly, and make a decision against an artifact that carries its own audit trail.

Cross-frontier links are the next composition. A finding in a BBB frontier should be able to point at a finding in an immune-cell-trafficking frontier and say *extends* or *contradicts* or *depends-on*. Once two frontiers exist, this is forced: someone will want it, and the protocol will need to encode it.

Hub-to-hub federation is the next discovery composition. Different communities will run different hubs. A computational-biology hub and a neurodegeneration hub will mirror each other's relevant `vfr_id`s the way different package registries mirror each other's tarballs. That follows once a second hub exists.

None of that matters until people are using the first one.

---

This is the part of the essay that wants to become a call to action. I'll resist that. There isn't a "join us" button. There's a public hub, a frontier you can pull, an SDK you can install, and source code you can read. If you are a domain scientist who has felt the friction I described in the opening, the contradictions that get resolved in showers and forgotten, pull the frontier and tell me where it falls down. If you are an agent team building scientific reasoning loops, you have a substrate that is not a chat interface, that handles signatures and content-addressing, that you can integrate against in an afternoon.

Science as state is not a feature. It is a way of working. The substrate is small enough now to be honest about what it is and isn't. It is not a lab runtime. It is not an autonomous agent. It is not the operating system for science. It is the missing layer underneath all of those things, and it is finally real enough to use.

Pull the frontier. Tell me what's wrong with it. That's the next move.

---

*The reference implementation lives at <https://github.com/vela-science/vela>. The public hub is at <https://vela-hub.fly.dev>. The protocol is documented at [docs/PROTOCOL.md](../docs/PROTOCOL.md), the registry primitive at [docs/REGISTRY.md](../docs/REGISTRY.md), and the hub at [docs/HUB.md](../docs/HUB.md).*
