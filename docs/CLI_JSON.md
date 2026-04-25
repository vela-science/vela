# Vela CLI JSON contracts

This document defines the stable `--json` output contracts for the strict core
Vela release. These contracts are for machine consumption by tests, demos,
agents, and release automation.

JSON output MUST be:

- valid UTF-8 JSON on stdout only
- free of ANSI color, progress text, tables, and prose wrappers
- one top-level JSON object per command
- deterministic for the same frontier input and command arguments, except for
  explicitly documented generated timestamps
- conservative about candidate outputs: gaps, tensions, bridges, observer views,
  and prior-art checks are navigation signals, not scientific conclusions

Errors MUST be emitted on stderr and use a non-zero exit code. If a command can
produce a structured JSON error, it SHOULD use:

```json
{
  "ok": false,
  "command": "check",
  "error": {
    "code": "frontier_load_failed",
    "message": "Failed to load frontier"
  }
}
```

The successful top-level envelope is:

```json
{
  "ok": true,
  "command": "stats",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  }
}
```

`frontier.hash` is the SHA-256 digest of the canonical frontier state used by
the command. For a monolithic `frontier.json`, hash the file bytes. For a
frontier directory, hash the deterministic manifest of included frontier files:
relative path, byte length, and file SHA-256, sorted by relative path.

## `vela stats <frontier> --json`

Returns aggregate frontier metadata and statistics.

```json
{
  "ok": true,
  "command": "stats",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "description": "Blood-brain barrier Alzheimer frontier",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:...",
    "compiled_at": "2026-04-22T00:00:00Z",
    "compiler": "vela/0.2.0",
    "papers_processed": 10,
    "errors": 0
  },
  "stats": {
    "findings": 48,
    "links": 121,
    "replicated": 12,
    "unreplicated": 36,
    "avg_confidence": 0.742,
    "gaps": 7,
    "negative_space": 2,
    "contested": 4,
    "human_reviewed": 3,
    "review_event_count": 1,
    "confidence_update_count": 0,
    "source_count": 10,
    "evidence_atom_count": 48,
    "condition_record_count": 48,
    "categories": {
      "mechanism": 24,
      "therapeutic": 9
    },
    "link_types": {
      "supports": 73,
      "contradicts": 5,
      "depends": 18
    },
    "confidence_distribution": {
      "high_gt_80": 11,
      "medium_60_80": 30,
      "low_lt_60": 7
    }
  }
}
```

Stable fields are `frontier`, `stats`, and all nested stat keys shown above.
Maps such as `categories` and `link_types` MAY contain additional keys.

## `vela check <frontier> --json`

Returns validation status for schema, stats lint, and optional conformance
checks. By default, `check` runs the release-safe checks for the supplied
frontier. Explicit flags narrow or expand the check set.

```json
{
  "ok": true,
  "command": "check",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "summary": {
    "status": "pass",
    "checked_findings": 48,
    "valid_findings": 48,
    "invalid_findings": 0,
    "errors": 0,
    "warnings": 0,
    "info": 0
  },
  "checks": [
    {
      "id": "schema",
      "status": "pass",
      "checked": 48,
      "failed": 0,
      "diagnostics": []
    },
    {
      "id": "stats",
      "status": "pass",
      "checked": 48,
      "failed": 0,
      "diagnostics": []
    }
  ],
  "event_log": {
    "count": 3,
    "kinds": {"finding.reviewed": 1, "finding.caveated": 1, "finding.confidence_revised": 1},
    "first_timestamp": "2026-04-22T00:00:00Z",
    "last_timestamp": "2026-04-22T00:00:00Z",
    "duplicate_ids": [],
    "orphan_targets": []
  },
  "replay": {
    "ok": true,
    "status": "ok",
    "baseline_hash": null,
    "replayed_hash": "sha256:...",
    "current_hash": "sha256:...",
    "conflicts": [],
    "applied_events": 3
  },
  "source_registry": {
    "count": 10,
    "source_types": {"paper": 8, "csv": 2},
    "low_quality_count": 0,
    "missing_hash_count": 8
  },
  "evidence_atoms": {
    "count": 48,
    "missing_locator_count": 0,
    "unverified_count": 44,
    "synthetic_source_count": 0
  },
  "conditions": {
    "count": 48,
    "missing_text_count": 0,
    "missing_comparator_count": 33,
    "exposure_efficacy_risk_count": 0,
    "translation_scopes": {
      "animal_model": 32,
      "human": 15,
      "in_vitro": 1
    }
  },
  "proposals": {
    "total": 2,
    "pending_review": 1,
    "accepted": 0,
    "rejected": 0,
    "applied": 1
  },
  "proof_state": {
    "latest_packet": {"status": "current"},
    "last_event_at_export": "2026-04-22T00:00:00Z",
    "stale_reason": null
  },
  "signals": [],
  "review_queue": [
    {
      "id": "rq_0123456789abcdef",
      "priority": "high",
      "priority_score": 120,
      "target": {"type": "finding", "id": "vf_0123456789abcdef"},
      "signal_ids": ["sig_missing_evidence_span_0123456789abcdef"],
      "reasons": ["Finding has no verified evidence span attached."],
      "recommended_action": "Verify the assertion against source text and add evidence spans where possible."
    }
  ],
  "proof_readiness": {
    "status": "ready",
    "blockers": 0,
    "warnings": 0,
    "caveats": []
  }
}
```

`summary.status` is one of `pass`, `warn`, or `fail`. `event_log` summarizes
canonical state events. `replay.status` is `ok`, `no_events`, or `conflict`.
`source_registry` summarizes source artifacts, while `evidence_atoms`
summarizes the source-grounded spans, rows, measurements, or weak provenance
atoms attached to findings. `proposals` summarizes the review queue, and
`proof_state` reports whether the latest proof packet is current, stale, or
missing. Conflicts fail `check`, and `--strict` also fails on blocking
proof-readiness signals. Diagnostics use this shape:

```json
{
  "severity": "error",
  "rule_id": "content_addressed_id",
  "finding_id": "vf_0123456789abcdef",
  "file": "vf_0123456789abcdef",
  "message": "Finding id does not match content-address",
  "suggestion": "Recompute the finding id from assertion and provenance"
}
```

`severity` is one of `error`, `warning`, or `info`.

## `vela proof <frontier> --out <dir> --json`

Builds and validates a proof packet. The JSON response summarizes the generated
packet and points to the deterministic proof trace described in
[`TRACE_FORMAT.md`](TRACE_FORMAT.md). By default this command does not write
back to the input frontier; `--record-proof-state` is an advanced local
bookkeeping flag that records `proof_state.latest_packet` after successful
packet validation.

```json
{
  "ok": true,
  "command": "proof",
  "schema_version": "0.2.0",
  "recorded_proof_state": false,
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "template": "bbb-alzheimer",
  "output": "proof-packet",
  "packet": {
    "manifest_path": "proof-packet/manifest.json"
  },
  "validation": {
    "status": "ok",
    "summary": "vela packet validate\n  root: proof-packet\n  status: ok\n  checked_files: 28\n  project: BBB Flagship"
  },
  "signals": [],
  "review_queue": [],
  "proof_readiness": {
    "status": "ready",
    "blockers": 0,
    "warnings": 0,
    "caveats": []
  },
  "trace_path": "proof-packet/proof-trace.json"
}
```

`validation.status` is `ok` or `failed`. A failed validation MUST exit non-zero.

## `vela compile ./papers --output frontier.json`

Local corpus compile writes JSON sidecars beside the output frontier:

- `compile-report.json`
- `quality-table.json`
- `frontier-quality.md`

This command is a bootstrap surface. Its output should be treated as candidate
frontier state until reviewed; accepted proposals and canonical events are the
durable trust boundary.

`compile-report.json` uses:

```json
{
  "schema": "vela.compile-report.v0",
  "command": "compile",
  "source": {
    "path": "./papers",
    "mode": "local_corpus"
  },
  "output": {
    "frontier": "frontier.json"
  },
  "summary": {
    "files_seen": 4,
    "accepted": 4,
    "skipped": 0,
    "errors": 0,
    "findings": 11,
    "links": 11
  },
  "source_coverage": {
    "csv": 1,
    "text": 1,
    "jats": 1,
    "pdf": 1
  },
  "extraction_modes": {
    "curated_csv": 1,
    "offline_text": 1
  },
  "sources": [
    {
      "path": "papers/example.pdf",
      "source_type": "pdf",
      "status": "accepted",
      "extraction_mode": "offline_pdf",
      "findings": 3,
      "diagnostics": {
        "page_count": 2,
        "text_chars": 424,
        "word_count": 61,
        "text_quality": "thin_text",
        "detected_title": "Example paper",
        "detected_doi": null,
        "caveats": ["pdf source has limited extractable text; verify evidence spans before use."]
      },
      "warnings": []
    }
  ],
  "warnings": [],
  "artifacts": {
    "compile_report": "compile-report.json",
    "quality_table": "quality-table.json",
    "frontier_quality": "frontier-quality.md"
  }
}
```

`quality-table.json` is a review aid with one row per finding. It is not a
scientific quality score. Rows include source file, span status, provenance
completeness, frontier confidence components, extraction confidence, entity
resolution status, caveats, and a recommended review action.

## `vela bench <frontier> --gold <file> --json`

Measures frontier drift against a frozen gold set. In finding mode this includes
extraction-alignment metrics, but passing the benchmark is release discipline,
not a claim that compile quality is the v0 proof.

```json
{
  "ok": true,
  "command": "bench",
  "benchmark_type": "finding",
  "mode": "finding",
  "suite_id": null,
  "task_id": null,
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "gold": {
    "path": "benchmarks/gold-50.json",
    "hash": "sha256:...",
    "items": 50
  },
  "metrics": {
    "total_frontier_findings": 48,
    "total_gold_findings": 8,
    "matched": 8,
    "total_frontier_matched": 8,
    "unmatched_gold": 0,
    "unmatched_frontier": 40,
    "exact_id_matches": 8,
    "precision": 0.167,
    "recall": 1.0,
    "f1": 0.286,
    "entity_accuracy": 1.0,
    "assertion_type_accuracy": 1.0,
    "confidence_calibration": 1.0
  },
  "thresholds": {
    "min_f1": 0.28,
    "min_precision": 0.15,
    "min_recall": 1.0
  },
  "failures": [],
  "match_details": [
    {
      "gold_id": "vf_...",
      "frontier_id": "vf_...",
      "gold_text": "LRP1 mediates amyloid beta efflux across the BBB",
      "frontier_text": "LRP1 supports amyloid beta clearance at the blood-brain barrier",
      "similarity": 0.429,
      "entity_overlap": 0.667,
      "assertion_type_match": true,
      "confidence_in_range": true,
      "exact_id_match": true
    }
  ]
}
```

For `--entity-gold`, set `benchmark_type` to `entity`. For `--link-gold`, set
`benchmark_type` to `link`. Those modes MUST keep the same envelope and put
mode-specific scores under `metrics`.

## `vela bench --suite <file> --json`

Runs a suite of finding, entity, link, and workflow benchmark tasks. Each task
uses the same single-mode envelope described above.

```json
{
  "ok": true,
  "command": "bench",
  "benchmark_type": "suite",
  "schema_version": "0.2.0",
  "suite": {
    "id": "bbb-alzheimer-quality-gate-v0",
    "name": "BBB Alzheimer Quality Gate",
    "path": "benchmarks/suites/bbb-core.json",
    "tasks": 4
  },
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "metrics": {
    "tasks_total": 4,
    "tasks_passed": 4,
    "tasks_failed": 0,
    "standard_candles": 14
  },
  "standard_candles": {
    "definition": "Reviewed gold fixtures used as calibration anchors for release drift, not proof of scientific superiority.",
    "items": []
  },
  "failures": [],
  "tasks": [
    {
      "ok": true,
      "command": "bench",
      "benchmark_type": "finding",
      "mode": "finding",
      "suite_id": "bbb-alzheimer-quality-gate-v0",
      "task_id": "bbb-findings",
      "metrics": {}
    }
  ]
}
```

`vela bench --suite <file> --suite-ready` returns a compact JSON readiness
report over the same suite tasks.

Benchmark scores are regression signals for a frozen fixture. They are not
claims about field-level scientific completeness.

## `vela serve <frontier> --check-tools --json`

Checks the read-only MCP/HTTP frontier tool surface and exits without starting a server. Without `--json`, the command prints a short human summary.

```json
{
  "ok": true,
  "command": "serve --check-tools",
  "schema": "vela.tool-check.v0",
  "frontier": {
    "name": "papers",
    "findings": 11,
    "links": 11
  },
  "summary": {
    "checks": 9,
    "passed": 9,
    "failed": 0
  },
  "tool_count": 9,
  "tools": [
    "frontier_stats",
    "search_findings",
    "list_gaps",
    "list_contradictions",
    "find_bridges",
    "apply_observer",
    "propagate_retraction",
    "get_finding",
    "trace_evidence_chain"
  ],
  "registered_tool_count": 10,
  "registered_tools": [
    "frontier_stats",
    "search_findings",
    "get_finding",
    "list_gaps",
    "list_contradictions",
    "find_bridges",
    "check_pubmed",
    "apply_observer",
    "propagate_retraction",
    "trace_evidence_chain"
  ],
  "checks": [
    {
      "tool": "frontier_stats",
      "ok": true,
      "data": {},
      "markdown": "{...}",
      "has_data": true,
      "has_markdown": true,
      "has_signals": true,
      "has_caveats": true,
      "signals": [],
      "caveats": [],
      "duration_ms": 0
    }
  ],
  "failures": []
}
```

## `vela search <query> --source <frontier> --json`

Returns ranked finding matches for a query.

```json
{
  "ok": true,
  "command": "search",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "query": "LRP1 RAGE amyloid",
  "filters": {
    "entity": null,
    "assertion_type": null,
    "limit": 20
  },
  "count": 2,
  "results": [
    {
      "id": "vf_0123456789abcdef",
      "score": 5.5,
      "assertion": "LRP1 supports amyloid beta efflux across the blood-brain barrier",
      "assertion_type": "mechanism",
      "confidence": 0.84,
      "entities": ["LRP1", "amyloid beta", "blood-brain barrier"],
      "doi": "10.0000/example"
    }
  ]
}
```

`score` is a search relevance score, not confidence. Result ordering is by
descending `score`; ties SHOULD be broken by finding ID for deterministic output.

## `vela tensions <frontier> --json`

Returns candidate contradiction/tension pairs.

```json
{
  "ok": true,
  "command": "tensions",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "filters": {
    "both_high": true,
    "cross_domain": false,
    "top": 20
  },
  "count": 1,
  "tensions": [
    {
      "score": 70.0,
      "resolved": false,
      "superseding_id": null,
      "finding_a": {
        "id": "vf_0123456789abcdef",
        "assertion": "RAGE increases amyloid beta influx at the BBB",
        "confidence": 0.9,
        "assertion_type": "mechanism",
        "citation_count": 50,
        "contradicts_count": 1
      },
      "finding_b": {
        "id": "vf_fedcba9876543210",
        "assertion": "RAGE blockade has no measurable amyloid transport effect",
        "confidence": 0.85,
        "assertion_type": "therapeutic",
        "citation_count": 32,
        "contradicts_count": 1
      },
      "caveat": "Candidate contradiction inferred from typed links; inspect both findings before treating it as resolved or unresolved."
    }
  ]
}
```

`score` is a prioritization heuristic. It must not be described as truth,
agreement, or severity without human review.

## `vela gaps rank <frontier> --json`

Returns candidate gap review-lead rankings. These are navigation signals over
flagged findings, not scientific conclusions or guaranteed experiment targets.

```json
{
  "ok": true,
  "command": "gaps rank",
  "schema_version": "0.2.0",
  "frontier": {
    "name": "bbb-flagship",
    "source": "frontiers/bbb-alzheimer.json",
    "hash": "sha256:..."
  },
  "filters": {
    "top": 5,
    "domain": null
  },
  "count": 1,
  "ranking_label": "candidate gap review leads",
  "caveats": [
    "These rankings are navigation signals over flagged findings, not scientific conclusions."
  ],
  "review_leads": [
    {
      "id": "vf_0123456789abcdef",
      "kind": "candidate_gap_review_lead",
      "assertion": "Longitudinal BBB transporter changes remain underexplored",
      "score": 3.4,
      "dependency_count": 5,
      "confidence": 0.68,
      "evidence_type": "observational",
      "entities": ["LRP1", "blood-brain barrier"],
      "recommended_action": "Review source scope and missing evidence before treating this as an experiment target.",
      "caveats": [
        "Candidate gap rankings are review leads, not guaranteed underexplored areas or experiment targets."
      ]
    }
  ],
  "gaps": [
    {
      "id": "vf_0123456789abcdef",
      "kind": "candidate_gap_review_lead",
      "assertion": "Longitudinal BBB transporter changes remain underexplored",
      "score": 3.4,
      "dependency_count": 5,
      "confidence": 0.68,
      "evidence_type": "observational",
      "entities": ["LRP1", "blood-brain barrier"],
      "recommended_action": "Review source scope and missing evidence before treating this as an experiment target.",
      "caveats": [
        "Candidate gap rankings are review leads, not guaranteed underexplored areas or experiment targets."
      ]
    }
  ]
}
```

`score` is a deterministic review-prioritization heuristic:

```text
dependency_count + finding confidence
```

Cost labels are rough planning placeholders and MUST NOT be treated as budget
estimates for release claims.

## State transition commands

The release write surface records durable frontier state transitions through
proposal-first writes. These commands do not delete history.

```bash
vela finding add frontier.json --assertion "..." --author reviewer:demo --json
vela review frontier.json vf_0123 --status contested --reason "..." --reviewer reviewer:demo --json
vela note frontier.json vf_0123 --text "..." --author reviewer:demo --json
vela caveat frontier.json vf_0123 --text "..." --author reviewer:demo --json
vela revise frontier.json vf_0123 --confidence 0.42 --reason "..." --reviewer reviewer:demo --json
vela reject frontier.json vf_0123 --reason "..." --reviewer reviewer:demo --json
vela retract frontier.json vf_0123 --reason "..." --reviewer reviewer:demo --json
vela proposals list frontier.json --status pending_review --json
vela proposals accept frontier.json vpr_0123456789abcdef --reviewer reviewer:demo --reason "Accepted after review" --json
vela history frontier.json vf_0123 --json
vela import-events packet-or-events.json --into frontier.json --json
```

`finding add`, `review`, `note`, `caveat`, `revise`, `reject`, and `retract` create
`vela.proposal.v0.1` records by default. `--apply` accepts and applies the
proposal locally in one step. Applied proposals append a canonical
`vela.event.v0.1` event and then save the materialized frontier snapshot. They
return this stable envelope:

```json
{
  "ok": true,
  "command": "finding.add",
  "frontier": "bbb-flagship",
  "finding_id": "vf_0123456789abcdef",
  "proposal_id": "vpr_0123456789abcdef",
  "proposal_status": "pending_review",
  "applied_event_id": null,
  "wrote_to": "frontier.json",
  "message": "Finding proposal recorded"
}
```

`history` returns the current finding snapshot plus canonical events,
compatibility review/confidence projections, and annotations:

```json
{
  "ok": true,
  "command": "history",
  "frontier": "bbb-flagship",
  "finding": {
    "id": "vf_0123456789abcdef",
    "assertion": "LRP1 clears amyloid beta at the BBB",
    "confidence": 0.42,
    "flags": {},
    "annotations": []
  },
  "events": [],
  "review_events": [],
  "confidence_updates": [],
  "proposals": []
}
```

Proof packets include canonical events at `events/events.json`, replay status at
`events/replay-report.json`, the combined derived log at
`state-transitions.json`, proposal records at `proposals/proposals.json`, and
compatibility projection files under `reviews/`.

## Release tests

Release tests should assert field presence and types, not exact pretty-printing.
The legacy `benchmark` command is intentionally absent; use `bench`.
