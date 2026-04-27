//! Content-addressed finding bundles — the atomic object of the Vela protocol.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Valid entity types per schema. Single source of truth shared by the validator
/// and the `vela finding add` CLI; do not duplicate.
///
/// v0.10 added domain-neutral entries — `particle`, `instrument`, `dataset`,
/// `quantity` — surfaced by the first non-bio frontier on the public hub
/// (Nakamura's dark-matter constraints). The biology-leaning entries remain
/// for back-compat; the additions widen expressiveness without churn.
pub const VALID_ENTITY_TYPES: &[&str] = &[
    // bio (pre-v0.10)
    "gene",
    "protein",
    "compound",
    "disease",
    "cell_type",
    "organism",
    "pathway",
    "assay",
    "anatomical_structure",
    // domain-neutral (v0.10)
    "particle",
    "instrument",
    "dataset",
    "quantity",
    // escape valve
    "other",
];

/// Valid assertion types per schema.
///
/// v0.10 added `measurement` and `exclusion` for measurement-heavy domains
/// (physics, chemistry, climate, materials) where the substance of a
/// finding is a numerical value or an exclusion limit at a confidence level.
pub const VALID_ASSERTION_TYPES: &[&str] = &[
    "mechanism",
    "therapeutic",
    "diagnostic",
    "epidemiological",
    "observational",
    "review",
    "methodological",
    "computational",
    "theoretical",
    "negative",
    // v0.10
    "measurement",
    "exclusion",
    // v0.30: Notes Compiler emits these for proposals derived from
    // researcher zettelkasten / Obsidian vaults. They become canonical
    // findings on accept; rejecting them at the validator would force a
    // post-hoc rewrite that breaks content-addressed ids. The semantic
    // intent: `tension` = a theoretical claim about a field-level
    // contradiction (paired claims that don't reconcile); `open_question`
    // = an unresolved framing the agent surfaced; `hypothesis` = a
    // provisional candidate claim awaiting evidence. The notes-compiler
    // proposals doc covers how these are produced.
    "tension",
    "open_question",
    "hypothesis",
    "candidate_finding",
];

/// Valid evidence types per schema.
pub const VALID_EVIDENCE_TYPES: &[&str] = &[
    "experimental",
    "observational",
    "computational",
    "theoretical",
    "meta_analysis",
    "systematic_review",
    "case_report",
    // v0.30: Notes Compiler — the evidence span lives in the researcher's
    // zettelkasten note rather than a primary literature passage.
    // Treated as an `expert_assertion`-shaped evidence kind.
    "extracted_from_notes",
];

/// Valid provenance source types per schema.
///
/// v0.10 added `data_release` for instrument runs, observation campaigns,
/// and dataset versions that are themselves the substantive object — distinct
/// from the paper that reports them (XENONnT SR0, Planck data releases,
/// JWST observation runs, LHC analysis releases).
pub const VALID_PROVENANCE_SOURCE_TYPES: &[&str] = &[
    "published_paper",
    "preprint",
    "clinical_trial",
    "lab_notebook",
    "model_output",
    "expert_assertion",
    "database_record",
    // v0.10
    "data_release",
    // v0.30: notes-compiler proposals cite the source markdown note
    // by filename. Distinct from `lab_notebook` (which implies a
    // dated lab workbook entry with primary observations) and
    // `expert_assertion` (which implies a named expert's claim).
    "researcher_notes",
];

/// Valid link types per protocol §5.
pub const VALID_LINK_TYPES: &[&str] = &[
    "supports",
    "contradicts",
    "extends",
    "depends",
    "replicates",
    "supersedes",
    "synthesized_from",
];

/// A resolved identifier from a scientific database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedId {
    /// The database source (mesh, uniprot, pubchem, chebi, go, ncbi_gene).
    pub source: String,
    /// The identifier value (e.g., "D000544", "Q6ZSS7", "24752728").
    pub id: String,
    /// Confidence in this resolution (0.0-1.0).
    pub confidence: f64,
    /// The matched name in the source database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_name: Option<String>,
}

/// How an entity was resolved to its canonical form (v0.2.0 schema).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMethod {
    ExactMatch,
    FuzzyMatch,
    LlmInference,
    Manual,
}

impl std::fmt::Display for ResolutionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionMethod::ExactMatch => write!(f, "exact_match"),
            ResolutionMethod::FuzzyMatch => write!(f, "fuzzy_match"),
            ResolutionMethod::LlmInference => write!(f, "llm_inference"),
            ResolutionMethod::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    /// Deprecated: flat identifiers map. Retained for backward compatibility with
    /// older frontier JSON files. New code should use `canonical_id` and `candidates`.
    #[serde(default)]
    pub identifiers: serde_json::Map<String, serde_json::Value>,
    /// The primary resolved identifier (if resolved).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_id: Option<ResolvedId>,
    /// Alternative resolution candidates with scores.
    #[serde(default)]
    pub candidates: Vec<ResolvedId>,
    /// Known aliases for this entity (e.g., NLRP3 = cryopyrin = NALP3).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// How this resolution was performed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_provenance: Option<String>,
    #[serde(default = "default_one")]
    pub resolution_confidence: f64,
    /// How the entity was resolved: exact_match, fuzzy_match, llm_inference, manual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_method: Option<ResolutionMethod>,
    /// Species context for orthologs (e.g., "Homo sapiens" vs "Mus musculus" for APP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub species_context: Option<String>,
    /// True when resolution_confidence < 0.8 and the match needs human review.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_review: bool,
}

fn default_one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(rename = "type")]
    pub evidence_type: String,
    #[serde(default)]
    pub model_system: String,
    pub species: Option<String>,
    #[serde(default)]
    pub method: String,
    pub sample_size: Option<String>,
    pub effect_size: Option<String>,
    pub p_value: Option<String>,
    #[serde(default)]
    pub replicated: bool,
    pub replication_count: Option<u32>,
    #[serde(default)]
    pub evidence_spans: Vec<serde_json::Value>,
}

/// Valid replication outcomes per v0.32 schema.
///
/// `replicated`: an independent attempt reproduced the finding within the
/// stated conditions. `failed`: the attempt did not reproduce. `partial`:
/// some conditions matched, others didn't (e.g., effect size present but
/// smaller). `inconclusive`: methodology ambiguity prevents a clean
/// outcome judgment.
pub const VALID_REPLICATION_OUTCOMES: &[&str] = &[
    "replicated",
    "failed",
    "partial",
    "inconclusive",
];

/// v0.32: Replication as a first-class kernel object.
///
/// Before v0.32, replication was encoded as `Evidence.replicated: bool`
/// + `Evidence.replication_count: u32` — a scalar property on the
/// finding. The kernel could not represent "lab A replicated this in
/// human iPSC; lab B failed to replicate in mouse OPCs" — those are
/// distinct epistemic facts, not a single count.
///
/// Each `Replication` is content-addressed (`vrep_<16hex>`) over its
/// target finding, the actor that attempted it, the canonical
/// conditions, and the outcome. This mirrors the `vf_<id>` pattern and
/// makes replication chains queryable, citeable, and propagable through
/// the link graph.
///
/// The legacy `Evidence.replicated` and `Evidence.replication_count`
/// fields are preserved for backward compatibility; v0.32+ frontiers
/// derive them from the structured collection on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replication {
    /// `vrep_<16hex>`, content-addressed; see `Replication::content_address`.
    pub id: String,
    /// `vf_<id>` of the finding being replicated.
    pub target_finding: String,
    /// Stable actor id of the lab / curator / agent that attempted the
    /// replication. Same shape as `FindingBundle.actor` references.
    pub attempted_by: String,
    /// One of `replicated`, `failed`, `partial`, `inconclusive`.
    /// Stored as a string for forward-compat with future outcome
    /// taxonomies; validated against `VALID_REPLICATION_OUTCOMES`.
    pub outcome: String,
    /// Evidence collected from the replication attempt. Reuses the
    /// existing `Evidence` shape so confidence math stays consistent.
    pub evidence: Evidence,
    /// Conditions under which the replication was attempted (model
    /// system, species, in_vivo/vitro, etc.). The conditions field is
    /// what makes "replicated in mouse but failed in human" a
    /// representable fact.
    pub conditions: Conditions,
    /// Provenance of the replicating paper / preprint / lab notebook.
    pub provenance: Provenance,
    /// Free-text reviewer note. Often the most important field for
    /// partial / inconclusive outcomes.
    #[serde(default)]
    pub notes: String,
    /// Original creation timestamp (RFC 3339).
    pub created: String,
    /// If this attempt extends or refines a previous one, the
    /// `vrep_<id>` of that earlier attempt. Allows replication chains
    /// (lab A → lab B refines → lab C generalizes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_attempt: Option<String>,
}

impl Replication {
    /// Compute the content-addressed ID per v0.32 spec:
    /// `SHA-256(target_finding | attempted_by | normalize(conditions.text) | outcome)`.
    /// Returns first 16 hex chars prefixed with "vrep_".
    ///
    /// `conditions.text` is normalized by the same lower/whitespace/punct
    /// rules as `FindingBundle::normalize_text` so two replications with
    /// trivially-different condition prose produce the same id only when
    /// the substantive conditions match.
    pub fn content_address(
        target_finding: &str,
        attempted_by: &str,
        conditions: &Conditions,
        outcome: &str,
    ) -> String {
        let norm_conditions = FindingBundle::normalize_text(&conditions.text);
        let preimage = format!(
            "{}|{}|{}|{}",
            target_finding, attempted_by, norm_conditions, outcome
        );
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vrep_{}", &hex::encode(hash)[..16])
    }

    /// Construct a new Replication with a freshly-derived id and
    /// `created` timestamp set to now.
    pub fn new(
        target_finding: impl Into<String>,
        attempted_by: impl Into<String>,
        outcome: impl Into<String>,
        evidence: Evidence,
        conditions: Conditions,
        provenance: Provenance,
        notes: impl Into<String>,
    ) -> Self {
        let target = target_finding.into();
        let actor = attempted_by.into();
        let oc = outcome.into();
        let id = Self::content_address(&target, &actor, &conditions, &oc);
        Self {
            id,
            target_finding: target,
            attempted_by: actor,
            outcome: oc,
            evidence,
            conditions,
            provenance,
            notes: notes.into(),
            created: Utc::now().to_rfc3339(),
            previous_attempt: None,
        }
    }
}

/// v0.34: ExpectedOutcome — the structured shape of a Prediction's
/// expected resolution.
///
/// `Affirmed` / `Falsified` are the binary cases ("this claim will
/// hold" / "this claim will fail"). `Quantitative` carries a numeric
/// expectation with tolerance + units ("CDR-SB effect ≥ 0.4 SD ± 0.1").
/// `Categorical` carries an arbitrary label for outcomes that aren't
/// numeric ("FDA decision is one of: full approval, accelerated,
/// declined").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpectedOutcome {
    Affirmed,
    Falsified,
    Quantitative {
        value: f64,
        tolerance: f64,
        units: String,
    },
    Categorical {
        value: String,
    },
}

impl ExpectedOutcome {
    /// Compact string representation used in the content-address
    /// preimage and CLI rendering.
    pub fn canonical(&self) -> String {
        match self {
            ExpectedOutcome::Affirmed => "affirmed".to_string(),
            ExpectedOutcome::Falsified => "falsified".to_string(),
            ExpectedOutcome::Quantitative {
                value,
                tolerance,
                units,
            } => format!("quant:{value}±{tolerance}{units}"),
            ExpectedOutcome::Categorical { value } => format!("cat:{value}"),
        }
    }
}

/// v0.34: Prediction as a first-class kernel object.
///
/// A `Prediction` is a falsifiable claim about a future observation,
/// scoped to one or more existing findings, made by a registered
/// actor at a known timestamp, with an explicit resolution
/// criterion and (typically) a deadline. Resolutions arrive later as
/// `Resolution` records that close out the prediction by recording
/// what actually happened.
///
/// Predictions are the kernel's epistemic accountability layer.
/// Other parts of the substrate describe what *is* believed today;
/// predictions describe what is *expected* and let the substrate
/// score, over time, how well each actor's beliefs track reality.
/// Calibration records (Brier, log score, hit rate) are derived
/// from the resolved subset.
///
/// `vpred_<id>` is content-addressed over `claim_text + made_by +
/// predicted_at + resolution_criterion`. Two predictions with the
/// same prose but different actors or different criteria are
/// distinct kernel objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// `vpred_<16hex>`, content-addressed.
    pub id: String,
    /// The falsifiable prediction itself, in plain prose.
    pub claim_text: String,
    /// Existing `vf_*` findings whose truth this prediction depends
    /// on. May be empty for predictions that don't tie back to a
    /// specific frontier claim.
    #[serde(default)]
    pub target_findings: Vec<String>,
    /// RFC 3339 timestamp of when the prediction was made. Goes into
    /// the content-address preimage so re-asserting the same prose
    /// at a later date produces a distinct record.
    pub predicted_at: String,
    /// RFC 3339 deadline for resolution. `None` means open-ended; a
    /// concrete date is strongly preferred for calibration scoring.
    pub resolves_by: Option<String>,
    /// Unambiguous prose that says "we'll know this resolved when X."
    /// Goes into the content-address preimage so the same prose with
    /// a different criterion is a distinct record.
    pub resolution_criterion: String,
    /// Structured expectation: affirmed / falsified / quantitative /
    /// categorical. The resolver checks this against the observed
    /// outcome at resolution time.
    pub expected_outcome: ExpectedOutcome,
    /// Stable actor id of the predictor.
    pub made_by: String,
    /// Predictor's prior belief in the expected outcome, on [0, 1].
    /// Drives Brier scoring at resolution time.
    pub confidence: f64,
    /// Conditions under which the prediction applies. Reuses the
    /// `Conditions` shape so model relevance, scope, etc., flow
    /// through.
    pub conditions: Conditions,
}

impl Prediction {
    /// Compute the content-addressed ID per v0.34 spec:
    /// `SHA-256(normalize(claim_text) | made_by | predicted_at | normalize(resolution_criterion) | expected_outcome.canonical())`.
    /// Returns first 16 hex chars prefixed with "vpred_".
    pub fn content_address(
        claim_text: &str,
        made_by: &str,
        predicted_at: &str,
        resolution_criterion: &str,
        expected_outcome: &ExpectedOutcome,
    ) -> String {
        let preimage = format!(
            "{}|{}|{}|{}|{}",
            FindingBundle::normalize_text(claim_text),
            made_by,
            predicted_at,
            FindingBundle::normalize_text(resolution_criterion),
            expected_outcome.canonical(),
        );
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vpred_{}", &hex::encode(hash)[..16])
    }

    /// Construct a new Prediction. `predicted_at` defaults to "now"
    /// in RFC 3339 if not supplied.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        claim_text: impl Into<String>,
        target_findings: Vec<String>,
        predicted_at: Option<String>,
        resolves_by: Option<String>,
        resolution_criterion: impl Into<String>,
        expected_outcome: ExpectedOutcome,
        made_by: impl Into<String>,
        confidence: f64,
        conditions: Conditions,
    ) -> Self {
        let now = predicted_at.unwrap_or_else(|| Utc::now().to_rfc3339());
        let claim = claim_text.into();
        let crit = resolution_criterion.into();
        let actor = made_by.into();
        let id = Self::content_address(&claim, &actor, &now, &crit, &expected_outcome);
        Self {
            id,
            claim_text: claim,
            target_findings,
            predicted_at: now,
            resolves_by,
            resolution_criterion: crit,
            expected_outcome,
            made_by: actor,
            confidence,
            conditions,
        }
    }
}

/// v0.34: Resolution closes out a Prediction.
///
/// A `Resolution` records what actually happened, who observed it,
/// when, with what evidence, and whether the actual outcome matched
/// the predicted one. Calibration scoring (Brier, log score, hit rate)
/// runs over the resolved subset of predictions per actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    /// `vres_<16hex>`, content-addressed.
    pub id: String,
    /// `vpred_<id>` of the prediction this resolves.
    pub prediction_id: String,
    /// Free-text description of what actually happened. The
    /// `matched_expected` flag is the structured judgment.
    pub actual_outcome: String,
    /// True if the observed outcome matched the prediction's
    /// `expected_outcome`. Drives hit-rate and Brier scoring.
    pub matched_expected: bool,
    /// RFC 3339 timestamp of resolution.
    pub resolved_at: String,
    /// Stable actor id of the resolver. May or may not be the same
    /// actor that made the prediction (independent resolution is
    /// stronger).
    pub resolved_by: String,
    /// Evidence supporting the resolution — typically the paper /
    /// trial readout / observation that closes out the bet.
    pub evidence: Evidence,
    /// Resolver's confidence in the match judgment, on [0, 1].
    /// Useful when the actual outcome is partial or ambiguous.
    pub confidence: f64,
}

impl Resolution {
    /// Compute the content-addressed ID per v0.34 spec:
    /// `SHA-256(prediction_id | normalize(actual_outcome) | resolved_by | resolved_at | matched)`.
    /// Returns first 16 hex chars prefixed with "vres_".
    pub fn content_address(
        prediction_id: &str,
        actual_outcome: &str,
        resolved_by: &str,
        resolved_at: &str,
        matched_expected: bool,
    ) -> String {
        let preimage = format!(
            "{}|{}|{}|{}|{}",
            prediction_id,
            FindingBundle::normalize_text(actual_outcome),
            resolved_by,
            resolved_at,
            matched_expected,
        );
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vres_{}", &hex::encode(hash)[..16])
    }

    /// Construct a Resolution with a freshly-derived id and `resolved_at`
    /// timestamp.
    pub fn new(
        prediction_id: impl Into<String>,
        actual_outcome: impl Into<String>,
        matched_expected: bool,
        resolved_by: impl Into<String>,
        evidence: Evidence,
        confidence: f64,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        let pid = prediction_id.into();
        let outcome = actual_outcome.into();
        let resolver = resolved_by.into();
        let id = Self::content_address(&pid, &outcome, &resolver, &now, matched_expected);
        Self {
            id,
            prediction_id: pid,
            actual_outcome: outcome,
            matched_expected,
            resolved_at: now,
            resolved_by: resolver,
            evidence,
            confidence,
        }
    }
}

/// v0.33: Dataset as a first-class kernel object.
///
/// A `Dataset` is a versioned, content-addressed reference to data
/// that anchors empirical claims. Before v0.33, datasets were strings
/// in `Provenance.title` or entity-typed mentions in assertions —
/// a claim could say "we used ADNI" without anchoring which release
/// of ADNI the analysis ran against, and re-running the same code on
/// a refreshed cohort silently produced a "different" claim.
///
/// `vd_<id>` is content-addressed over `name + version + content_hash
/// + url`. Two dataset records with the same name but different
/// versions get distinct ids; two records pointing at the same
/// snapshot collapse to the same id. This is the substrate piece
/// that makes "Git for science" mean something operational rather
/// than aspirational — claims literally reference the bytes they
/// rest on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    /// `vd_<16hex>`, content-addressed; see `Dataset::content_address`.
    pub id: String,
    /// Human-readable name (e.g. "ADNI", "TRAILBLAZER-ALZ", "MIMIC-IV").
    pub name: String,
    /// Semantic version or release tag (e.g. "ADNI-3", "v2.2", "SR0").
    /// Two entries differing only in version are distinct kernel objects.
    pub version: Option<String>,
    /// Optional column-level schema as `(name, type)` pairs. For
    /// non-tabular datasets, leave empty.
    #[serde(default)]
    pub schema: Vec<(String, String)>,
    /// Number of rows / observations / records, when known.
    pub row_count: Option<u64>,
    /// SHA-256 of the canonical contents, when computable. For
    /// large datasets stored remotely, this is the publisher's
    /// declared content hash; integrity verification is the puller's
    /// job (same pattern as `vfr_*` snapshots).
    pub content_hash: String,
    /// Where the dataset is reachable (https URL, file://, s3://, etc.).
    pub url: Option<String>,
    /// License identifier or URL (e.g. "CC-BY-4.0", a Crossref license).
    pub license: Option<String>,
    /// Provenance of the dataset itself — typically the paper or release
    /// that publishes it. Reuses `Provenance` for shape parity with
    /// findings.
    pub provenance: Provenance,
    /// RFC 3339 creation timestamp.
    pub created: String,
}

impl Dataset {
    /// Compute the content-addressed ID per v0.33 spec:
    /// `SHA-256(name | version | content_hash | url)`.
    /// Returns first 16 hex chars prefixed with "vd_".
    pub fn content_address(
        name: &str,
        version: Option<&str>,
        content_hash: &str,
        url: Option<&str>,
    ) -> String {
        let preimage = format!(
            "{}|{}|{}|{}",
            name,
            version.unwrap_or(""),
            content_hash,
            url.unwrap_or("")
        );
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vd_{}", &hex::encode(hash)[..16])
    }

    /// Construct a new Dataset with a freshly-derived id and `created`
    /// timestamp set to now.
    pub fn new(
        name: impl Into<String>,
        version: Option<String>,
        content_hash: impl Into<String>,
        url: Option<String>,
        license: Option<String>,
        provenance: Provenance,
    ) -> Self {
        let n = name.into();
        let h = content_hash.into();
        let id = Self::content_address(&n, version.as_deref(), &h, url.as_deref());
        Self {
            id,
            name: n,
            version,
            schema: Vec::new(),
            row_count: None,
            content_hash: h,
            url,
            license,
            provenance,
            created: Utc::now().to_rfc3339(),
        }
    }
}

/// v0.33: CodeArtifact as a first-class kernel object.
///
/// A `CodeArtifact` is a content-addressed pointer at a specific
/// region of source code (a function, a notebook cell, a script, a
/// pipeline step) at a specific git commit. Before v0.33, code was
/// captured as a string in `Evidence.method` — "we ran a logistic
/// regression" — with no way for a reader to verify which code
/// produced the result, or to re-run it.
///
/// `vc_<id>` is content-addressed over `repo_url + git_commit + path
/// + line_range + content_hash`. The same code at two commits gets
/// two records (the relevant historical fact); the same code in two
/// paths in the same repo also gets two records (location matters
/// for re-execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeArtifact {
    /// `vc_<16hex>`, content-addressed; see `CodeArtifact::content_address`.
    pub id: String,
    /// Source language: `python` / `r` / `julia` / `rust` / `bash`,
    /// etc. Not validated against a closed allow-list — code provenance
    /// should accept whatever language the analysis was actually in.
    pub language: String,
    /// Repository URL (e.g. `https://github.com/vela-science/vela`).
    pub repo_url: Option<String>,
    /// Specific git commit (40-char SHA preferred). Required for
    /// reproducibility; `None` means "unpinned" and weakens the
    /// substrate claim.
    pub git_commit: Option<String>,
    /// Path within the repository (e.g. `crates/vela-scientist/src/notes.rs`).
    pub path: String,
    /// Optional line range as `(start, end)`, both inclusive.
    pub line_range: Option<(u32, u32)>,
    /// SHA-256 of the snippet body. Decouples the artifact from the
    /// repository's external state — even if a repo is deleted, the
    /// content_hash remains anchored.
    pub content_hash: String,
    /// Optional entry point: function name, notebook cell id, or
    /// `__main__`. Used by re-execution tooling.
    pub entry_point: Option<String>,
    /// RFC 3339 creation timestamp.
    pub created: String,
}

impl CodeArtifact {
    /// Compute the content-addressed ID per v0.33 spec:
    /// `SHA-256(repo_url | git_commit | path | line_range | content_hash)`.
    /// Returns first 16 hex chars prefixed with "vc_".
    pub fn content_address(
        repo_url: Option<&str>,
        git_commit: Option<&str>,
        path: &str,
        line_range: Option<(u32, u32)>,
        content_hash: &str,
    ) -> String {
        let lr = line_range
            .map(|(a, b)| format!("{a}-{b}"))
            .unwrap_or_default();
        let preimage = format!(
            "{}|{}|{}|{}|{}",
            repo_url.unwrap_or(""),
            git_commit.unwrap_or(""),
            path,
            lr,
            content_hash
        );
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vc_{}", &hex::encode(hash)[..16])
    }

    /// Construct a new CodeArtifact with a freshly-derived id and
    /// `created` timestamp.
    pub fn new(
        language: impl Into<String>,
        repo_url: Option<String>,
        git_commit: Option<String>,
        path: impl Into<String>,
        line_range: Option<(u32, u32)>,
        content_hash: impl Into<String>,
        entry_point: Option<String>,
    ) -> Self {
        let p = path.into();
        let h = content_hash.into();
        let id = Self::content_address(
            repo_url.as_deref(),
            git_commit.as_deref(),
            &p,
            line_range,
            &h,
        );
        Self {
            id,
            language: language.into(),
            repo_url,
            git_commit,
            path: p,
            line_range,
            content_hash: h,
            entry_point,
            created: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conditions {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub species_verified: Vec<String>,
    #[serde(default)]
    pub species_unverified: Vec<String>,
    #[serde(default)]
    pub in_vitro: bool,
    #[serde(default)]
    pub in_vivo: bool,
    #[serde(default)]
    pub human_data: bool,
    #[serde(default)]
    pub clinical_trial: bool,
    pub concentration_range: Option<String>,
    pub duration: Option<String>,
    pub age_group: Option<String>,
    pub cell_type: Option<String>,
}

/// Structured breakdown of frontier epistemic confidence (v0.2.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceComponents {
    /// Derived from evidence.type (meta_analysis=0.95, systematic_review=0.90,
    /// experimental=0.80, observational=0.65, computational=0.55, case_report=0.40,
    /// theoretical=0.30).
    #[serde(alias = "evidence_grade")]
    pub evidence_strength: f64,
    /// 1.0 if replicated with high count, 0.7 if not replicated.
    /// When replicated: min(1.0, 0.7 + 0.1 * replication_count).
    #[serde(alias = "replication_factor")]
    pub replication_strength: f64,
    /// Derived from sample_size: >1000 -> 1.0, >100 -> 0.9, >30 -> 0.8,
    /// >10 -> 0.7, <=10 or null -> 0.6.
    pub sample_strength: f64,
    /// human_data=1.0, in_vivo=0.8, in_vitro=0.6, else=0.5.
    #[serde(alias = "species_relevance")]
    pub model_relevance: f64,
    /// Reduces score when finding is contested. 0.15 if contested, else 0.0.
    #[serde(alias = "contradiction_penalty")]
    pub review_penalty: f64,
    /// Additive calibration signal layered on top of the deterministic support score.
    #[serde(default)]
    pub calibration_adjustment: f64,
    /// Confidence formula version stamp. v0.3 introduced this; v0.4
    /// bumps it to "v0.4" for the same scoring formula recomputed
    /// against substrate-level changes (genesis events, signed actors,
    /// canonical/derived split — none of which alter scoring math).
    /// A second implementation may refuse to interpret components
    /// computed with an unknown formula version.
    #[serde(default = "default_formula_version")]
    pub formula_version: String,
}

fn default_formula_version() -> String {
    "v0.8".to_string()
}

/// Confidence method: how the score was determined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ConfidenceMethod {
    /// Computed from structured frontier support components (v0.2.0).
    Computed,
    /// A human expert assigned it.
    ExpertJudgment,
    /// Legacy import path for confidence seeded before component breakdown existed.
    #[default]
    LlmInitial,
}

/// Semantic category of the confidence score stored on the frontier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceKind {
    /// Bounded epistemic support for the finding as currently represented in frontier state.
    #[default]
    FrontierEpistemic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confidence {
    /// Semantic meaning of `score`. v0 emits `frontier_epistemic`.
    #[serde(default)]
    pub kind: ConfidenceKind,
    pub score: f64,
    pub basis: String,
    /// How this score was determined.
    #[serde(default)]
    pub method: ConfidenceMethod,
    /// Structured component breakdown required by the current schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<ConfidenceComponents>,
    /// Confidence in the extraction itself (separate from scientific confidence).
    #[serde(default = "default_extraction_conf")]
    pub extraction_confidence: f64,
}

fn default_extraction_conf() -> f64 {
    0.85
}

impl Confidence {
    /// Construct a `Confidence` with a raw score and basis string but
    /// without the structured `components` breakdown. The agent layer
    /// uses this when an LLM produces a single confidence value; the
    /// computed deterministic components arrive later via
    /// `compute_confidence`.
    ///
    /// Renamed from `legacy()` in v0.36; the previous name was a
    /// historical accident (the constructor was never actually
    /// deprecated, just misnamed when the structured components shipped
    /// alongside it).
    pub fn raw(score: f64, basis: impl Into<String>, extraction_confidence: f64) -> Self {
        Self {
            kind: ConfidenceKind::FrontierEpistemic,
            score,
            basis: basis.into(),
            method: ConfidenceMethod::LlmInitial,
            components: None,
            extraction_confidence,
        }
    }
}

/// Parse a sample_size string into a numeric value for scoring.
/// Handles formats like "n=30", "n = 120", "3 cohorts of 20", "500", "n=24 per group".
fn parse_sample_size(s: &str) -> Option<u64> {
    let mut max_num: Option<u64> = None;
    for word in s.split(|c: char| !c.is_ascii_digit()) {
        if let Ok(n) = word.parse::<u64>() {
            max_num = Some(max_num.map_or(n, |prev: u64| prev.max(n)));
        }
    }
    max_num
}

/// Compute frontier epistemic confidence from evidence and condition fields.
/// Returns a fully populated Confidence with components and aggregate score,
/// using a deterministic, auditable support computation.
///
/// Back-compat wrapper: derives `n_replicated` from the legacy
/// `Evidence.replicated` / `Evidence.replication_count` scalars, with
/// `n_failed` and `n_partial` defaulting to zero. Use
/// `Project::compute_confidence_for` when the v0.32 `Replication`
/// collection is available — that's the authoritative path.
pub fn compute_confidence(
    evidence: &Evidence,
    conditions: &Conditions,
    contested: bool,
) -> Confidence {
    let n_replicated = if evidence.replicated {
        evidence.replication_count.unwrap_or(1)
    } else {
        0
    };
    compute_confidence_from_components(evidence, conditions, contested, n_replicated, 0, 0)
}

/// Pure-math kernel for the frontier-epistemic confidence formula. Takes
/// replication counts as inputs so the same math drives both the legacy
/// scalar path (`compute_confidence`) and the v0.32 Project-aware path
/// (`Project::compute_confidence_for`).
///
/// Replication strength schedule:
/// `clamp(0.7 + 0.1 * n_replicated + 0.05 * n_partial - 0.10 * n_failed, 0.4, 1.0)`
///
/// Floor at 0.4 keeps a single failed replication from zeroing out the
/// computation; ceiling at 1.0 caps the bonus from accumulated successes.
/// `inconclusive` outcomes do not move the score (deliberate — they
/// represent methodological ambiguity, not evidence).
#[must_use]
pub fn compute_confidence_from_components(
    evidence: &Evidence,
    conditions: &Conditions,
    contested: bool,
    n_replicated: u32,
    n_failed: u32,
    n_partial: u32,
) -> Confidence {
    let evidence_strength = match evidence.evidence_type.as_str() {
        "meta_analysis" => 0.95,
        "systematic_review" => 0.90,
        "experimental" => 0.80,
        "observational" => 0.65,
        "computational" => 0.55,
        "case_report" => 0.40,
        "theoretical" => 0.30,
        _ => 0.50,
    };

    let replication_strength = (0.7
        + 0.1 * f64::from(n_replicated)
        + 0.05 * f64::from(n_partial)
        - 0.10 * f64::from(n_failed))
    .clamp(0.4, 1.0);

    let sample_strength = match evidence.sample_size.as_deref().and_then(parse_sample_size) {
        Some(n) if n > 1000 => 1.0,
        Some(n) if n > 100 => 0.9,
        Some(n) if n > 30 => 0.8,
        Some(n) if n > 10 => 0.7,
        Some(_) => 0.6,
        None => 0.6,
    };

    let model_relevance = if conditions.human_data {
        1.0
    } else if conditions.in_vivo {
        0.8
    } else if conditions.in_vitro {
        0.6
    } else {
        0.5
    };

    let review_penalty = if contested { 0.15 } else { 0.0 };
    let calibration_adjustment = 0.0;

    let raw = evidence_strength * replication_strength * model_relevance * sample_strength
        - review_penalty
        + calibration_adjustment;
    let score = raw.clamp(0.0, 1.0);
    let score = (score * 1000.0).round() / 1000.0;

    let components = ConfidenceComponents {
        evidence_strength,
        replication_strength,
        sample_strength,
        model_relevance,
        review_penalty,
        calibration_adjustment,
        formula_version: "v0.6".to_string(),
    };

    let basis = format!(
        "frontier_epistemic: evidence={:.2} * replication={:.2} * model={:.2} * sample={:.2} - review_penalty={:.2} + calibration={:.2} = {:.3}",
        evidence_strength,
        replication_strength,
        model_relevance,
        sample_strength,
        review_penalty,
        calibration_adjustment,
        score,
    );

    Confidence {
        kind: ConfidenceKind::FrontierEpistemic,
        score,
        basis,
        method: ConfidenceMethod::Computed,
        components: Some(components),
        extraction_confidence: default_extraction_conf(),
    }
}

/// Count v0.32 replication outcomes targeting a given finding id.
/// Returns `(n_replicated, n_failed, n_partial)`. Inconclusive outcomes
/// are deliberately excluded — they represent methodological ambiguity
/// and don't move the confidence score.
#[must_use]
pub fn count_replication_outcomes(
    replications: &[Replication],
    target_finding: &str,
) -> (u32, u32, u32) {
    let mut n_replicated = 0u32;
    let mut n_failed = 0u32;
    let mut n_partial = 0u32;
    for r in replications {
        if r.target_finding != target_finding {
            continue;
        }
        match r.outcome.as_str() {
            "replicated" => n_replicated += 1,
            "failed" => n_failed += 1,
            "partial" => n_partial += 1,
            _ => {}
        }
    }
    (n_replicated, n_failed, n_partial)
}

/// Recompute confidence scores for all findings in a slice using the
/// v0.32 `Replication` collection as the source of truth. Returns the
/// number of findings whose score changed by more than 0.001.
///
/// When `replications` is empty (e.g., legacy frontiers pre-v0.32), the
/// math falls back through `compute_confidence_from_components` with
/// counts derived from the scalar `Evidence.replicated` /
/// `Evidence.replication_count` fields, preserving prior behavior.
pub fn recompute_all_confidence(
    findings: &mut [FindingBundle],
    replications: &[Replication],
) -> usize {
    let mut changed = 0;
    for bundle in findings.iter_mut() {
        let old_score = bundle.confidence.score;
        let extraction_conf = bundle.confidence.extraction_confidence;
        let (n_repl, n_failed, n_partial) =
            count_replication_outcomes(replications, &bundle.id);
        // If the v0.32 collection has nothing for this finding, fall back
        // to the legacy scalar so unmigrated frontiers keep their prior
        // computed confidence.
        let (n_repl, n_failed, n_partial) = if n_repl + n_failed + n_partial == 0 {
            let legacy = if bundle.evidence.replicated {
                bundle.evidence.replication_count.unwrap_or(1)
            } else {
                0
            };
            (legacy, 0, 0)
        } else {
            (n_repl, n_failed, n_partial)
        };
        let mut new_conf = compute_confidence_from_components(
            &bundle.evidence,
            &bundle.conditions,
            bundle.flags.contested,
            n_repl,
            n_failed,
            n_partial,
        );
        // Preserve the extraction confidence from the original extraction.
        new_conf.extraction_confidence = extraction_conf;
        if (new_conf.score - old_score).abs() > 0.001 {
            changed += 1;
        }
        bundle.confidence = new_conf;
    }
    changed
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extraction {
    #[serde(default = "default_extraction_method")]
    pub method: String,
    pub model: Option<String>,
    pub model_version: Option<String>,
    #[serde(default)]
    pub extracted_at: String,
    #[serde(default = "default_extractor_version")]
    pub extractor_version: String,
}

fn default_extraction_method() -> String {
    "llm_extraction".into()
}
fn default_extractor_version() -> String {
    "vela/0.2.0".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    #[serde(default)]
    pub reviewed: bool,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub corrections: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub orcid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(default = "default_source_type")]
    pub source_type: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmc: Option<String>,
    pub openalex_id: Option<String>,
    /// v0.11: generic source URL when none of the structured identifiers
    /// fit (preprint server URL, dataset landing page, talk recording, etc.).
    /// Skipped when None so pre-v0.11 frontiers serialise byte-identically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub authors: Vec<Author>,
    pub year: Option<i32>,
    pub journal: Option<String>,
    /// License URL (e.g., Creative Commons), typically from Crossref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Publisher name, typically from Crossref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Funding sources, typically from Crossref.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funders: Vec<String>,
    #[serde(default)]
    pub extraction: Extraction,
    pub review: Option<Review>,
    /// Citation count of the source paper (from OpenAlex).
    #[serde(default)]
    pub citation_count: Option<u64>,
}

fn default_source_type() -> String {
    "published_paper".into()
}

/// Typed review state. Replaces the v0.2 `flags.contested: bool` collapse
/// of three semantically distinct review judgments. Doctrine line 6:
/// "scientific disagreement should remain live state."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Review verdict was "accepted" or "approved" — finding stands.
    Accepted,
    /// Review verdict was "contested" — disagreement preserved as live state.
    Contested,
    /// Review verdict was "needs_revision" — finding stays but flagged for
    /// confidence revision or condition refinement.
    NeedsRevision,
    /// Review verdict was "rejected" — finding kept for replay history but
    /// not treated as active state.
    Rejected,
}

impl ReviewState {
    /// Whether `flags.contested` should be true given this review_state.
    /// Backwards-compat shim: contested is the v0.2 derived bit.
    #[must_use]
    pub fn implies_contested(&self) -> bool {
        matches!(
            self,
            ReviewState::Contested | ReviewState::NeedsRevision | ReviewState::Rejected
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flags {
    #[serde(default)]
    pub gap: bool,
    #[serde(default)]
    pub negative_space: bool,
    /// Derived from `review_state` for backward compatibility. Code that
    /// reads `flags.contested` still works; new code should read
    /// `review_state` for the typed verdict.
    #[serde(default)]
    pub contested: bool,
    #[serde(default)]
    pub retracted: bool,
    #[serde(default)]
    pub declining: bool,
    #[serde(default)]
    pub gravity_well: bool,
    /// Typed review verdict (v0.3+). When set, drives `flags.contested`
    /// for backward compatibility. `None` means no review verdict has
    /// been recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_state: Option<ReviewState>,
    /// v0.14: true once a newer content-addressed finding supersedes
    /// this one via the `finding.supersede` proposal kind. The newer
    /// finding carries a `supersedes` link back to this finding's id.
    /// Skipped when false so pre-v0.14 frontiers serialize byte-identically.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub superseded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub text: String,
    #[serde(rename = "type")]
    pub assertion_type: String,
    #[serde(default)]
    pub entities: Vec<Entity>,
    pub relation: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub target: String,
    #[serde(rename = "type")]
    pub link_type: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "default_compiler")]
    pub inferred_by: String,
    /// When this link was created (immutable timestamp). Uses serde default for backward compat.
    #[serde(default)]
    pub created_at: String,
}

fn default_compiler() -> String {
    "compiler".into()
}

/// v0.8: typed reference resolved from `Link.target`.
///
/// Targets stay opaque `String` on the wire (canonical-JSON stable). At
/// validation/render time callers parse via `LinkRef::parse`. The
/// `Local` variant is the v0–v0.7 shape; `Cross` is new in v0.8 and
/// requires the dependent frontier to declare a matching `vfr_id` in
/// `frontier.dependencies`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRef {
    /// `vf_<16hex>` — the target finding lives in this same frontier.
    Local { vf_id: String },
    /// `vf_<16hex>@vfr_<16hex>` — the target finding lives in a
    /// different frontier. Strict validation requires the `vfr_id` to
    /// appear in `Project.frontier.dependencies`.
    Cross { vf_id: String, vfr_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkParseError {
    Empty,
    BadVfPrefix,
    BadVfrPrefix,
    EmptyVfId,
    EmptyVfrId,
    TooManyAtSigns,
}

impl std::fmt::Display for LinkParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkParseError::Empty => write!(f, "empty link target"),
            LinkParseError::BadVfPrefix => write!(f, "link target must start with 'vf_'"),
            LinkParseError::BadVfrPrefix => {
                write!(f, "cross-frontier suffix must start with 'vfr_'")
            }
            LinkParseError::EmptyVfId => write!(f, "link target's vf_ id is empty"),
            LinkParseError::EmptyVfrId => write!(f, "cross-frontier vfr_ id is empty"),
            LinkParseError::TooManyAtSigns => {
                write!(f, "link target has more than one '@' separator")
            }
        }
    }
}

impl std::error::Error for LinkParseError {}

impl LinkRef {
    /// Parse `vf_<id>` or `vf_<id>@vfr_<id>` into a typed reference.
    /// Treats inputs as opaque hex-ish blobs — does not validate hex
    /// length or character set, since the substrate's content-address
    /// derivation already handles that.
    pub fn parse(s: &str) -> Result<Self, LinkParseError> {
        if s.is_empty() {
            return Err(LinkParseError::Empty);
        }
        let mut parts = s.split('@');
        let local = parts.next().ok_or(LinkParseError::Empty)?;
        let remote = parts.next();
        if parts.next().is_some() {
            return Err(LinkParseError::TooManyAtSigns);
        }
        let vf_id = local
            .strip_prefix("vf_")
            .ok_or(LinkParseError::BadVfPrefix)?;
        if vf_id.is_empty() {
            return Err(LinkParseError::EmptyVfId);
        }
        match remote {
            None => Ok(LinkRef::Local {
                vf_id: local.to_string(),
            }),
            Some(r) => {
                let vfr_id = r.strip_prefix("vfr_").ok_or(LinkParseError::BadVfrPrefix)?;
                if vfr_id.is_empty() {
                    return Err(LinkParseError::EmptyVfrId);
                }
                Ok(LinkRef::Cross {
                    vf_id: local.to_string(),
                    vfr_id: r.to_string(),
                })
            }
        }
    }

    /// Round-trip: format back to the canonical wire string.
    pub fn format(&self) -> String {
        match self {
            LinkRef::Local { vf_id } => vf_id.clone(),
            LinkRef::Cross { vf_id, vfr_id } => format!("{vf_id}@{vfr_id}"),
        }
    }

    /// True if this reference points outside the current frontier.
    pub fn is_cross_frontier(&self) -> bool {
        matches!(self, LinkRef::Cross { .. })
    }
}

#[cfg(test)]
mod link_ref_tests {
    use super::*;

    #[test]
    fn parses_local_vf_id() {
        let r = LinkRef::parse("vf_abc123").unwrap();
        assert_eq!(
            r,
            LinkRef::Local {
                vf_id: "vf_abc123".into()
            }
        );
        assert_eq!(r.format(), "vf_abc123");
        assert!(!r.is_cross_frontier());
    }

    #[test]
    fn parses_cross_frontier_target() {
        let r = LinkRef::parse("vf_abc@vfr_def").unwrap();
        assert_eq!(
            r,
            LinkRef::Cross {
                vf_id: "vf_abc".into(),
                vfr_id: "vfr_def".into(),
            }
        );
        assert_eq!(r.format(), "vf_abc@vfr_def");
        assert!(r.is_cross_frontier());
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(LinkRef::parse(""), Err(LinkParseError::Empty));
    }

    #[test]
    fn rejects_missing_vf_prefix() {
        assert_eq!(LinkRef::parse("xx_abc"), Err(LinkParseError::BadVfPrefix));
    }

    #[test]
    fn rejects_empty_vf_id() {
        assert_eq!(LinkRef::parse("vf_"), Err(LinkParseError::EmptyVfId));
    }

    #[test]
    fn rejects_missing_vfr_prefix_after_at() {
        assert_eq!(
            LinkRef::parse("vf_abc@xxx_def"),
            Err(LinkParseError::BadVfrPrefix)
        );
    }

    #[test]
    fn rejects_empty_vfr_id() {
        assert_eq!(
            LinkRef::parse("vf_abc@vfr_"),
            Err(LinkParseError::EmptyVfrId)
        );
    }

    #[test]
    fn rejects_double_at() {
        assert_eq!(
            LinkRef::parse("vf_abc@vfr_def@x"),
            Err(LinkParseError::TooManyAtSigns)
        );
    }

    #[test]
    fn round_trips_real_ids() {
        for s in [
            "vf_d0a962d3251133dd",
            "vf_d0a962d3251133dd@vfr_7344e96c0f2669d5",
        ] {
            assert_eq!(LinkRef::parse(s).unwrap().format(), s);
        }
    }
}

/// A lightweight annotation on a finding — like a comment on a line of code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    /// Content-addressed ID (ann_{hash}).
    pub id: String,
    /// The annotation text.
    pub text: String,
    /// Who wrote it (ORCID preferred).
    pub author: String,
    /// When it was created (RFC 3339).
    pub timestamp: String,
    /// Phase β (v0.6): structured provenance for the annotation.
    /// Optional. When present, encodes which paper / preprint / extract
    /// span produced this note. Reviewers query by these fields:
    /// "show every annotation from PMID 25378646" works because the
    /// identifier is structure, not prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceRef>,
}

/// Phase β (v0.6): structured provenance reference attached to an
/// annotation (or any future note-shaped object). At least one
/// identifying field (`doi`, `pmid`, `title`) must be set when the
/// provenance is present; an all-empty `ProvenanceRef` is rejected by
/// `validate_event_payload`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Verbatim quote / extraction span from the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<String>,
}

impl ProvenanceRef {
    /// True iff at least one identifying field is set. Used by
    /// `validate_event_payload` to reject all-empty `provenance: {}` objects.
    #[must_use]
    pub fn has_identifier(&self) -> bool {
        self.doi.is_some() || self.pmid.is_some() || self.title.is_some()
    }
}

/// A file attached to a finding (dataset, figure, supplementary material).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub label: Option<String>,
    pub path: String,
    pub size_bytes: u64,
    pub mime_type: Option<String>,
    pub attached_at: String,
    pub attached_by: Option<String>,
}

// ── REVIEW layer: content-addressed review events ──────────────────────────

/// A review event is a content-addressed record of human judgment on a finding.
/// Like a Git commit, it records who, when, what changed, and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewEvent {
    /// Content-addressed ID of this review event.
    pub id: String,
    /// Optional workspace-relative origin for repo-scoped reviews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// ID of the finding being reviewed.
    pub finding_id: String,
    /// The reviewer (ORCID preferred).
    pub reviewer: String,
    /// When the review happened (RFC 3339).
    pub reviewed_at: String,
    /// Optional review scope for richer curation workflows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Optional status for the review event (for example: accepted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// What action was taken.
    pub action: ReviewAction,
    /// Human-readable reason.
    #[serde(default)]
    pub reason: String,
    /// Supporting findings or artifacts considered during review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_considered: Vec<ReviewEvidence>,
    /// Optional structured interpretation update payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_change: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewEvidence {
    pub finding_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The action taken in a review event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewAction {
    /// Finding approved as correct.
    Approved,
    /// Finding interpretation was qualified to narrow or constrain the claim.
    Qualified { target: String },
    /// Finding corrected — a specific field was changed.
    Corrected {
        field: String,
        original: String,
        corrected: String,
    },
    /// Finding flagged with a specific flag type.
    Flagged { flag_type: String },
    /// Finding disputed — reviewer disagrees with the claim.
    Disputed {
        counter_evidence: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter_doi: Option<String>,
    },
}

// ── Interpretation layer: mutable confidence updates ───────────────────────

/// A confidence update is a mutable interpretation layer event.
/// The finding's evidence is immutable; the confidence assessment can evolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceUpdate {
    pub finding_id: String,
    pub previous_score: f64,
    pub new_score: f64,
    pub basis: String,
    /// Who or what produced this update (e.g., "grounding_pass", "reviewer:0000-0001-2345-6789").
    pub updated_by: String,
    /// When this update was produced (RFC 3339).
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingBundle {
    pub id: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub previous_version: Option<String>,
    pub assertion: Assertion,
    pub evidence: Evidence,
    pub conditions: Conditions,
    pub confidence: Confidence,
    pub provenance: Provenance,
    pub flags: Flags,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    pub created: String,
    pub updated: Option<String>,
}

fn default_version() -> u32 {
    1
}

impl FindingBundle {
    /// Create a new finding bundle with a content-addressed ID.
    /// Normalize text for content-addressing: lowercase, collapse whitespace,
    /// strip trailing punctuation. Matches the v0.2.0 schema specification.
    /// Public since v0.32 so `Replication::content_address` can reuse the
    /// same canonicalization rule for its conditions preimage.
    pub fn normalize_text(s: &str) -> String {
        let lower = s.to_lowercase();
        // Collapse all runs of whitespace into a single space
        let collapsed: String = lower.split_whitespace().collect::<Vec<_>>().join(" ");
        // Strip trailing punctuation (., ;, :, !, ?)
        collapsed
            .trim_end_matches(['.', ';', ':', '!', '?'])
            .to_string()
    }

    /// Compute the content-addressed ID per v0.2.0 spec:
    /// SHA-256(normalize(assertion.text) + "|" + assertion.type + "|" + (provenance.doi || provenance.pmid || provenance.title))
    /// Returns first 16 hex chars prefixed with "vf_".
    pub fn content_address(assertion: &Assertion, provenance: &Provenance) -> String {
        let norm_text = Self::normalize_text(&assertion.text);
        let prov_id = provenance
            .doi
            .as_deref()
            .or(provenance.pmid.as_deref())
            .unwrap_or(&provenance.title);
        let preimage = format!("{}|{}|{}", norm_text, assertion.assertion_type, prov_id);
        let hash = Sha256::digest(preimage.as_bytes());
        format!("vf_{}", &hex::encode(hash)[..16])
    }

    pub fn new(
        assertion: Assertion,
        evidence: Evidence,
        conditions: Conditions,
        confidence: Confidence,
        provenance: Provenance,
        flags: Flags,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        let id = Self::content_address(&assertion, &provenance);

        Self {
            id,
            version: 1,
            previous_version: None,
            assertion,
            evidence,
            conditions,
            confidence,
            provenance,
            flags,
            links: Vec::new(),
            annotations: Vec::new(),
            attachments: Vec::new(),
            created: now,
            updated: None,
        }
    }

    pub fn add_link(&mut self, target_id: &str, link_type: &str, note: &str) {
        self.links.push(Link {
            target: target_id.to_string(),
            link_type: link_type.to_string(),
            note: note.to_string(),
            inferred_by: "compiler".to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
    }

    pub fn add_link_with_source(
        &mut self,
        target_id: &str,
        link_type: &str,
        note: &str,
        inferred_by: &str,
    ) {
        self.links.push(Link {
            target: target_id.to_string(),
            link_type: link_type.to_string(),
            note: note.to_string(),
            inferred_by: inferred_by.to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_assertion() -> Assertion {
        Assertion {
            text: "NLRP3 activates IL-1B".into(),
            assertion_type: "mechanism".into(),
            entities: vec![Entity {
                name: "NLRP3".into(),
                entity_type: "protein".into(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: vec![],
                aliases: vec![],
                resolution_provenance: None,
                resolution_confidence: 1.0,
                resolution_method: None,
                species_context: None,
                needs_review: false,
            }],
            relation: Some("activates".into()),
            direction: Some("positive".into()),
        }
    }

    fn sample_evidence() -> Evidence {
        Evidence {
            evidence_type: "experimental".into(),
            model_system: "mouse".into(),
            species: Some("Mus musculus".into()),
            method: "Western blot".into(),
            sample_size: Some("n=30".into()),
            effect_size: None,
            p_value: Some("p<0.05".into()),
            replicated: true,
            replication_count: Some(3),
            evidence_spans: vec![],
        }
    }

    fn sample_conditions() -> Conditions {
        Conditions {
            text: "In vitro, mouse microglia".into(),
            species_verified: vec!["Mus musculus".into()],
            species_unverified: vec![],
            in_vitro: true,
            in_vivo: false,
            human_data: false,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: Some("microglia".into()),
        }
    }

    fn sample_confidence() -> Confidence {
        Confidence {
            kind: ConfidenceKind::FrontierEpistemic,
            score: 0.85,
            basis: "Experimental with replication".into(),
            method: ConfidenceMethod::LlmInitial,
            components: None,
            extraction_confidence: 0.9,
        }
    }

    fn sample_provenance() -> Provenance {
        Provenance {
            source_type: "published_paper".into(),
            doi: Some("10.1234/test".into()),
            pmid: None,
            pmc: None,
            openalex_id: None,
            url: None,
            title: "Test Paper".into(),
            authors: vec![Author {
                name: "Smith J".into(),
                orcid: None,
            }],
            year: Some(2024),
            journal: Some("Nature".into()),
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Extraction::default(),
            review: None,
            citation_count: Some(100),
        }
    }

    fn sample_flags() -> Flags {
        Flags {
            gap: false,
            negative_space: false,
            contested: false,
            retracted: false,
            declining: false,
            gravity_well: false,
            review_state: None,
            superseded: false,
        }
    }

    // ── Content-addressed ID tests ───────────────────────────────────

    #[test]
    fn same_content_same_id() {
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let b2 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert_eq!(b1.id, b2.id);
    }

    #[test]
    fn different_content_different_id() {
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let mut different_assertion = sample_assertion();
        different_assertion.text = "Completely different claim".into();
        let b2 = FindingBundle::new(
            different_assertion,
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert_ne!(b1.id, b2.id);
    }

    #[test]
    fn id_starts_with_vf_prefix() {
        let b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert!(b.id.starts_with("vf_"));
        assert_eq!(b.id.len(), 3 + 16); // "vf_" + 16 hex chars
    }

    #[test]
    fn new_bundle_version_is_one() {
        let b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert_eq!(b.version, 1);
        assert!(b.previous_version.is_none());
    }

    #[test]
    fn new_bundle_has_no_links() {
        let b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert!(b.links.is_empty());
    }

    #[test]
    fn new_bundle_has_created_timestamp() {
        let b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert!(!b.created.is_empty());
        assert!(b.updated.is_none());
    }

    // ── add_link tests ───────────────────────────────────────────────

    #[test]
    fn add_link_works() {
        let mut b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        b.add_link("target_id", "extends", "shared entity");
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].target, "target_id");
        assert_eq!(b.links[0].link_type, "extends");
        assert_eq!(b.links[0].note, "shared entity");
        assert_eq!(b.links[0].inferred_by, "compiler");
    }

    #[test]
    fn add_link_with_source_works() {
        let mut b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        b.add_link_with_source(
            "target_id",
            "contradicts",
            "opposite direction",
            "entity_overlap",
        );
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].inferred_by, "entity_overlap");
    }

    #[test]
    fn multiple_links_accumulate() {
        let mut b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        b.add_link("t1", "extends", "note1");
        b.add_link("t2", "contradicts", "note2");
        b.add_link("t3", "supports", "note3");
        assert_eq!(b.links.len(), 3);
    }

    // ── ReviewEvent creation test ────────────────────────────────────

    #[test]
    fn review_event_creation() {
        let event = ReviewEvent {
            id: "rev_abc123".into(),
            workspace: None,
            finding_id: "vf_abc".into(),
            reviewer: "0000-0001-2345-6789".into(),
            reviewed_at: "2024-01-01T00:00:00Z".into(),
            scope: None,
            status: None,
            action: ReviewAction::Approved,
            reason: "Looks correct".into(),
            evidence_considered: vec![],
            state_change: None,
        };
        assert_eq!(event.finding_id, "vf_abc");
        assert_eq!(event.reviewer, "0000-0001-2345-6789");
    }

    #[test]
    fn review_action_corrected() {
        let action = ReviewAction::Corrected {
            field: "direction".into(),
            original: "positive".into(),
            corrected: "negative".into(),
        };
        if let ReviewAction::Corrected {
            field,
            original,
            corrected,
        } = action
        {
            assert_eq!(field, "direction");
            assert_eq!(original, "positive");
            assert_eq!(corrected, "negative");
        } else {
            panic!("Expected Corrected variant");
        }
    }

    #[test]
    fn review_action_disputed() {
        let action = ReviewAction::Disputed {
            counter_evidence: "Later study contradicts".into(),
            counter_doi: Some("10.1234/counter".into()),
        };
        if let ReviewAction::Disputed {
            counter_evidence,
            counter_doi,
        } = action
        {
            assert_eq!(counter_evidence, "Later study contradicts");
            assert_eq!(counter_doi, Some("10.1234/counter".into()));
        } else {
            panic!("Expected Disputed variant");
        }
    }

    // ── ConfidenceUpdate creation test ───────────────────────────────

    #[test]
    fn confidence_update_creation() {
        let update = ConfidenceUpdate {
            finding_id: "vf_abc".into(),
            previous_score: 0.7,
            new_score: 0.85,
            basis: "grounded".into(),
            updated_by: "grounding_pass".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        };
        assert_eq!(update.previous_score, 0.7);
        assert_eq!(update.new_score, 0.85);
        assert_eq!(update.updated_by, "grounding_pass");
    }

    // ── Serialization round-trip test ────────────────────────────────

    #[test]
    fn finding_serializes_and_deserializes() {
        let b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let json = serde_json::to_string(&b).unwrap();
        let b2: FindingBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(b.id, b2.id);
        assert_eq!(b.assertion.text, b2.assertion.text);
        assert_eq!(b.confidence.score, b2.confidence.score);
    }

    #[test]
    fn valid_entity_types_list() {
        // Pre-v0.10 (bio) entries
        for t in ["gene", "protein", "compound", "other"] {
            assert!(VALID_ENTITY_TYPES.contains(&t), "missing {t}");
        }
        // v0.10 domain-neutral additions
        for t in ["particle", "instrument", "dataset", "quantity"] {
            assert!(VALID_ENTITY_TYPES.contains(&t), "missing {t}");
        }
        assert_eq!(VALID_ENTITY_TYPES.len(), 14);
    }

    #[test]
    fn v0_10_assertion_and_source_extensions() {
        assert!(VALID_ASSERTION_TYPES.contains(&"measurement"));
        assert!(VALID_ASSERTION_TYPES.contains(&"exclusion"));
        assert!(VALID_PROVENANCE_SOURCE_TYPES.contains(&"data_release"));
    }

    // ── Different fields change the ID ───────────────────────────────

    #[test]
    fn confidence_does_not_affect_id() {
        // v0.2.0: confidence is the mutable interpretation layer, not part of content address
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let mut conf2 = sample_confidence();
        conf2.score = 0.5;
        let b2 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            conf2,
            sample_provenance(),
            sample_flags(),
        );
        assert_eq!(b1.id, b2.id);
    }

    #[test]
    fn flags_do_not_affect_id() {
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let mut flags2 = sample_flags();
        flags2.gap = true;
        flags2.contested = true;
        let b2 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            flags2,
        );
        // Flags are NOT in the content hash, so IDs should be the same
        assert_eq!(b1.id, b2.id);
    }

    #[test]
    fn different_assertion_text_different_id() {
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let mut assertion2 = sample_assertion();
        assertion2.assertion_type = "therapeutic".into();
        let b2 = FindingBundle::new(
            assertion2,
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        assert_ne!(b1.id, b2.id);
    }

    #[test]
    fn different_doi_different_id() {
        let b1 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        let mut prov2 = sample_provenance();
        prov2.doi = Some("10.5678/other".into());
        let b2 = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            prov2,
            sample_flags(),
        );
        assert_ne!(b1.id, b2.id);
    }

    // ── v0.2.0 content-addressing determinism ───────────────────────

    #[test]
    fn content_address_is_deterministic_across_runs() {
        // Two independent extraction runs with the same assertion text,
        // assertion type, and DOI must produce the same finding ID.
        let assertion1 = Assertion {
            text: "Mitochondrial dysfunction precedes amyloid plaque formation.".into(),
            assertion_type: "mechanism".into(),
            entities: vec![],
            relation: None,
            direction: None,
        };
        let prov1 = Provenance {
            source_type: "published_paper".into(),
            doi: Some("10.1038/s41586-023-06789-1".into()),
            pmid: None,
            pmc: None,
            openalex_id: None,
            url: None,
            title: "Mitochondria in AD".into(),
            authors: vec![],
            year: Some(2023),
            journal: None,
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Extraction::default(),
            review: None,
            citation_count: None,
        };

        // Different entities, evidence, conditions, confidence -- should NOT matter
        let assertion2 = Assertion {
            text: "Mitochondrial dysfunction precedes amyloid plaque formation.".into(),
            assertion_type: "mechanism".into(),
            entities: vec![Entity {
                name: "mitochondria".into(),
                entity_type: "anatomical_structure".into(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: vec![],
                aliases: vec![],
                resolution_provenance: None,
                resolution_confidence: 1.0,
                resolution_method: None,
                species_context: None,
                needs_review: false,
            }],
            relation: Some("precedes".into()),
            direction: Some("positive".into()),
        };
        let prov2 = Provenance {
            source_type: "published_paper".into(),
            doi: Some("10.1038/s41586-023-06789-1".into()),
            pmid: Some("37654321".into()),
            pmc: None,
            openalex_id: None,
            url: None,
            title: "Different title".into(),
            authors: vec![Author {
                name: "Jones A".into(),
                orcid: None,
            }],
            year: Some(2023),
            journal: Some("Nature".into()),
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Extraction::default(),
            review: None,
            citation_count: Some(50),
        };

        let id1 = FindingBundle::content_address(&assertion1, &prov1);
        let id2 = FindingBundle::content_address(&assertion2, &prov2);
        assert_eq!(
            id1, id2,
            "Same assertion text + type + DOI must produce same ID"
        );
    }

    #[test]
    fn content_address_normalizes_whitespace_and_punctuation() {
        let assertion1 = Assertion {
            text: "  NLRP3  activates   IL-1B.  ".into(),
            assertion_type: "mechanism".into(),
            entities: vec![],
            relation: None,
            direction: None,
        };
        let assertion2 = Assertion {
            text: "NLRP3 activates IL-1B".into(),
            assertion_type: "mechanism".into(),
            entities: vec![],
            relation: None,
            direction: None,
        };
        let prov = sample_provenance();
        let id1 = FindingBundle::content_address(&assertion1, &prov);
        let id2 = FindingBundle::content_address(&assertion2, &prov);
        assert_eq!(
            id1, id2,
            "Whitespace and trailing punctuation should be normalized away"
        );
    }

    #[test]
    fn content_address_falls_back_to_title_when_no_doi_or_pmid() {
        let assertion = sample_assertion();
        let mut prov = sample_provenance();
        prov.doi = None;
        prov.pmid = None;
        prov.title = "Fallback Title".into();
        let id = FindingBundle::content_address(&assertion, &prov);
        assert!(id.starts_with("vf_"));
        assert_eq!(id.len(), 19); // "vf_" + 16 hex chars

        // Same title -> same ID
        let mut prov2 = sample_provenance();
        prov2.doi = None;
        prov2.pmid = None;
        prov2.title = "Fallback Title".into();
        let id2 = FindingBundle::content_address(&assertion, &prov2);
        assert_eq!(id, id2);
    }

    #[test]
    fn content_address_prefers_doi_over_pmid_over_title() {
        let assertion = sample_assertion();

        let mut prov_doi = sample_provenance();
        prov_doi.doi = Some("10.1234/test".into());
        prov_doi.pmid = Some("12345".into());
        prov_doi.title = "Title".into();

        let mut prov_pmid = sample_provenance();
        prov_pmid.doi = None;
        prov_pmid.pmid = Some("12345".into());
        prov_pmid.title = "Title".into();

        let mut prov_title = sample_provenance();
        prov_title.doi = None;
        prov_title.pmid = None;
        prov_title.title = "Title".into();

        let id_doi = FindingBundle::content_address(&assertion, &prov_doi);
        let id_pmid = FindingBundle::content_address(&assertion, &prov_pmid);
        let id_title = FindingBundle::content_address(&assertion, &prov_title);

        // All three should be different since the provenance component differs
        assert_ne!(id_doi, id_pmid, "DOI vs PMID should differ");
        assert_ne!(id_pmid, id_title, "PMID vs title should differ");
        assert_ne!(id_doi, id_title, "DOI vs title should differ");
    }

    // ── compute_confidence tests ────────────────────────────────────

    #[test]
    fn compute_confidence_meta_analysis_human() {
        let evidence = Evidence {
            evidence_type: "meta_analysis".into(),
            model_system: "human cohorts".into(),
            species: Some("Homo sapiens".into()),
            method: "meta-analysis".into(),
            sample_size: Some("n=5000".into()),
            effect_size: None,
            p_value: None,
            replicated: true,
            replication_count: Some(5),
            evidence_spans: vec![],
        };
        let conditions = Conditions {
            text: String::new(),
            species_verified: vec![],
            species_unverified: vec![],
            in_vitro: false,
            in_vivo: false,
            human_data: true,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };
        let conf = compute_confidence(&evidence, &conditions, false);
        assert_eq!(conf.method, ConfidenceMethod::Computed);
        assert_eq!(conf.kind, ConfidenceKind::FrontierEpistemic);
        assert!(conf.components.is_some());
        let c = conf.components.unwrap();
        assert!((c.evidence_strength - 0.95).abs() < 0.001);
        assert!((c.replication_strength - 1.0).abs() < 0.001); // 0.7 + 0.1*5 = 1.2 -> clamped to 1.0
        assert!((c.sample_strength - 1.0).abs() < 0.001); // >1000
        assert!((c.model_relevance - 1.0).abs() < 0.001); // human_data
        assert!((c.review_penalty - 0.0).abs() < 0.001);
        assert!((c.calibration_adjustment - 0.0).abs() < 0.001);
        // 0.95 * 1.0 * 1.0 * 1.0 - 0.0 = 0.95
        assert!((conf.score - 0.95).abs() < 0.001);
    }

    #[test]
    fn compute_confidence_theoretical_no_replication() {
        let evidence = Evidence {
            evidence_type: "theoretical".into(),
            model_system: "computational".into(),
            species: None,
            method: "simulation".into(),
            sample_size: None,
            effect_size: None,
            p_value: None,
            replicated: false,
            replication_count: None,
            evidence_spans: vec![],
        };
        let conditions = Conditions {
            text: String::new(),
            species_verified: vec![],
            species_unverified: vec![],
            in_vitro: false,
            in_vivo: false,
            human_data: false,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };
        let conf = compute_confidence(&evidence, &conditions, false);
        let c = conf.components.unwrap();
        assert!((c.evidence_strength - 0.30).abs() < 0.001);
        assert!((c.replication_strength - 0.70).abs() < 0.001);
        assert!((c.sample_strength - 0.60).abs() < 0.001);
        assert!((c.model_relevance - 0.50).abs() < 0.001);
        // 0.30 * 0.70 * 0.50 * 0.60 = 0.063
        assert!((conf.score - 0.063).abs() < 0.001);
    }

    #[test]
    fn compute_confidence_contested_penalty() {
        let evidence = Evidence {
            evidence_type: "experimental".into(),
            model_system: "mouse".into(),
            species: Some("Mus musculus".into()),
            method: "Western blot".into(),
            sample_size: Some("n=30".into()),
            effect_size: None,
            p_value: None,
            replicated: false,
            replication_count: None,
            evidence_spans: vec![],
        };
        let conditions = Conditions {
            text: String::new(),
            species_verified: vec![],
            species_unverified: vec![],
            in_vitro: false,
            in_vivo: true,
            human_data: false,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };
        let uncontested = compute_confidence(&evidence, &conditions, false);
        let contested = compute_confidence(&evidence, &conditions, true);
        assert!((contested.score - (uncontested.score - 0.15)).abs() < 0.001);
    }

    #[test]
    fn compute_confidence_sample_size_parsing() {
        assert_eq!(parse_sample_size("n=30"), Some(30));
        assert_eq!(parse_sample_size("n = 120"), Some(120));
        assert_eq!(parse_sample_size("3 cohorts of 20"), Some(20));
        assert_eq!(parse_sample_size("500"), Some(500));
        assert_eq!(parse_sample_size(""), None);
    }

    #[test]
    fn compute_confidence_v010_deserialize_compat() {
        // Simulate an older JSON confidence object (no method, no components).
        let json = r#"{"score": 0.75, "basis": "legacy seeded confidence", "extraction_confidence": 0.85}"#;
        let conf: Confidence = serde_json::from_str(json).unwrap();
        assert!((conf.score - 0.75).abs() < 0.001);
        assert_eq!(conf.kind, ConfidenceKind::FrontierEpistemic);
        assert_eq!(conf.method, ConfidenceMethod::LlmInitial); // default
        assert!(conf.components.is_none());
    }

    #[test]
    fn compute_confidence_components_deserialize_legacy_names() {
        let json = r#"{
            "score": 0.75,
            "basis": "legacy components",
            "method": "computed",
            "components": {
                "evidence_grade": 0.8,
                "replication_factor": 0.7,
                "sample_strength": 0.6,
                "species_relevance": 0.8,
                "contradiction_penalty": 0.15
            },
            "extraction_confidence": 0.85
        }"#;
        let conf: Confidence = serde_json::from_str(json).unwrap();
        let components = conf.components.unwrap();
        assert!((components.evidence_strength - 0.8).abs() < 0.001);
        assert!((components.replication_strength - 0.7).abs() < 0.001);
        assert!((components.sample_strength - 0.6).abs() < 0.001);
        assert!((components.model_relevance - 0.8).abs() < 0.001);
        assert!((components.review_penalty - 0.15).abs() < 0.001);
        assert!((components.calibration_adjustment - 0.0).abs() < 0.001);
    }

    #[test]
    fn compute_confidence_serializes_new_component_names_and_kind() {
        let conf = compute_confidence(&sample_evidence(), &sample_conditions(), false);
        let value = serde_json::to_value(&conf).unwrap();
        assert_eq!(value["kind"], "frontier_epistemic");
        let components = &value["components"];
        assert!(components.get("evidence_strength").is_some());
        assert!(components.get("replication_strength").is_some());
        assert!(components.get("model_relevance").is_some());
        assert!(components.get("review_penalty").is_some());
        assert!(components.get("calibration_adjustment").is_some());
        assert!(components.get("evidence_grade").is_none());
        assert!(components.get("replication_factor").is_none());
        assert!(components.get("species_relevance").is_none());
        assert!(components.get("contradiction_penalty").is_none());
    }

    #[test]
    fn recompute_all_updates_findings() {
        let mut b = FindingBundle::new(
            sample_assertion(),
            sample_evidence(),
            sample_conditions(),
            sample_confidence(),
            sample_provenance(),
            sample_flags(),
        );
        // Original score is a seeded prior. The computed frontier support should differ.
        let old_score = b.confidence.score;
        assert!((old_score - 0.85).abs() < 0.001);
        let changed = recompute_all_confidence(std::slice::from_mut(&mut b), &[]);
        assert_eq!(b.confidence.method, ConfidenceMethod::Computed);
        assert!(b.confidence.components.is_some());
        // experimental=0.80, replicated(3)=min(1.0,0.7+0.3)=1.0, in_vitro=0.6, sample=n=30 (not >30)->0.7
        // 0.80 * 1.0 * 0.6 * 0.7 = 0.336
        assert!((b.confidence.score - 0.336).abs() < 0.001);
        assert_eq!(changed, 1);
    }

    #[test]
    fn parses_bbb_review_event_with_richer_schema() {
        let raw = include_str!("../../../tests/fixtures/legacy/rev_001_bbb_correction.json");
        let review: ReviewEvent = serde_json::from_str(raw).unwrap();

        assert_eq!(review.id, "rev_001_bbb_correction");
        assert_eq!(review.workspace.as_deref(), Some("projects/bbb-flagship"));
        assert_eq!(review.scope.as_deref(), Some("bbb_opening_trusted_subset"));
        assert_eq!(review.status.as_deref(), Some("accepted"));
        assert!(matches!(
            review.action,
            ReviewAction::Qualified { ref target } if target == "trusted_interpretation"
        ));
        assert_eq!(review.evidence_considered.len(), 3);
        assert_eq!(
            review.evidence_considered[0].role.as_deref(),
            Some("qualifier")
        );
        assert_eq!(
            review
                .state_change
                .as_ref()
                .and_then(|value| value.get("assumption_retired"))
                .and_then(|value| value.as_str()),
            Some("safe opening implies therapeutic efficacy")
        );
    }
}
