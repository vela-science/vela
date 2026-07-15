//! `vela atlas` — derived Math Atlas views (spec `docs/research/MATH_ATLAS.md`).
//!
//! Atlas commands never mint canonical Vela events. They project one or more
//! frontiers, inspect boundaries and blast radii, or materialize a regenerable
//! declaration-graph artifact. Source adapters are pure producers: package
//! their output as artifacts in `vela.receipt.v1`, then submit it through
//! `vela land` so policy and human custody remain on the one write edge.

use std::path::Path;

use serde_json::json;
use vela_protocol::{
    atlas, boundary,
    frontier_graph::{BlastDirection, EdgeKind, FrontierGraph},
    repo,
};

use crate::cli::{fail, fail_usage, print_json};

const RETIRED_INGEST_HELP: &str = "Atlas projections are read-only. Source adapters emit `vela.receipt.v1`; attach their output as receipt artifacts, then use `vela land <receipt.json> --frontier <frontier>`.";

/// Entry from the `cli.rs::run_from_args` intercept.
pub(crate) fn run(args: &[String]) {
    if matches!(
        args.get(2).map(String::as_str),
        Some("ingest" | "ingest-source" | "ingest-graph")
    ) {
        fail_usage(RETIRED_INGEST_HELP);
    }
    if args.get(2).map(String::as_str) == Some("boundary") {
        run_boundary(args);
        return;
    }
    if args.get(2).map(String::as_str) == Some("blast-radius") {
        run_blast_radius(args);
        return;
    }
    if args.get(2).map(String::as_str) == Some("decl-build") {
        run_decl_build(args);
        return;
    }
    if args.get(2).map(String::as_str) == Some("decl-blast") {
        run_decl_blast(args);
        return;
    }
    if args.get(2).map(String::as_str) == Some("gluing") {
        run_gluing(args);
        return;
    }
    if args.get(2).map(String::as_str) == Some("domains") {
        run_domains(args);
        return;
    }
    let frontiers: Vec<&str> = args
        .iter()
        .skip(2)
        .filter(|a| !a.starts_with('-'))
        .map(String::as_str)
        .collect();
    if frontiers.is_empty() {
        fail_usage(&format!(
            "usage: vela atlas <frontier> [<frontier> ...]\n{RETIRED_INGEST_HELP}"
        ));
    }
    let projects: Vec<_> = frontiers
        .iter()
        .map(|f| {
            repo::load_from_path(Path::new(f)).unwrap_or_else(|e| fail(&format!("load {f}: {e}")))
        })
        .collect();
    let refs: Vec<&_> = projects.iter().collect();
    let out = atlas::project(&refs);
    print_json(&serde_json::to_value(&out).unwrap_or_else(|e| fail(&format!("serialize: {e}"))));
}

/// `vela atlas domains <frontier>... --domains-of <map.json>` — project the
/// per-domain frontier state (frontier calculus lifted from a single claim to a
/// whole field). `--domains-of` is a JSON object mapping an Erdős problem id to
/// its domains (`{"102": ["additive combinatorics", "sidon sets"], ...}`); each
/// atlas cell is attributed to its problem's domains and the cells' bilattice
/// points are folded by `join_k`. Emits the `DomainAtlas`.
fn run_domains(args: &[String]) {
    let mut frontiers: Vec<&str> = Vec::new();
    let mut domains_of: Option<&str> = None;
    let mut i = 3; // after "atlas domains"
    while i < args.len() {
        if args[i] == "--domains-of" {
            domains_of = args.get(i + 1).map(String::as_str);
            i += 2;
            continue;
        }
        if !args[i].starts_with('-') {
            frontiers.push(&args[i]);
        }
        i += 1;
    }
    let usage =
        "usage: vela atlas domains <frontier> [<frontier> ...] --domains-of <problem-domains.json>";
    let domains_of = domains_of.unwrap_or_else(|| fail(usage));
    if frontiers.is_empty() {
        fail(usage);
    }
    let raw = std::fs::read_to_string(domains_of)
        .unwrap_or_else(|e| fail(&format!("read {domains_of}: {e}")));
    let map: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(&raw).unwrap_or_else(|e| fail(&format!("parse {domains_of}: {e}")));
    let projects: Vec<_> = frontiers
        .iter()
        .map(|f| {
            repo::load_from_path(Path::new(f)).unwrap_or_else(|e| fail(&format!("load {f}: {e}")))
        })
        .collect();
    let refs: Vec<&_> = projects.iter().collect();
    let atlas = atlas::project(&refs);
    let out = atlas::project_domains(&atlas, &map);
    print_json(&serde_json::to_value(&out).unwrap_or_else(|e| fail(&format!("serialize: {e}"))));
}

/// The digits that follow `keyword` (ASCII, case-insensitive) in `text`, after
/// skipping up to `max_skip` non-digit separators. e.g. `("erdos", 2)` finds the
/// number in "Erdos257", "erdos_257", "Erdős-642" (ASCII match on "erdos").
/// failing with a usage message when absent. Shared by the read-side verbs.
fn load_one(args: &[String], usage: &str) -> vela_protocol::project::Project {
    let frontier = args
        .iter()
        .skip(3)
        .find(|a| !a.starts_with('-'))
        .unwrap_or_else(|| fail(usage));
    repo::load_from_path(Path::new(frontier))
        .unwrap_or_else(|e| fail(&format!("load {frontier}: {e}")))
}

/// `vela atlas boundary <frontier>` — the dark-matter boundary (memo §3):
/// one-premise-away, fragile, contested, stale-open. Each item points at a
/// real finding a submission can be opened against.
fn run_boundary(args: &[String]) {
    let project = load_one(args, "usage: vela atlas boundary <frontier>");
    print_json(&boundary::Boundary::derive(&project).to_json());
}

/// `vela atlas blast-radius <frontier> <finding> [--impact up|down|both]
/// [--kinds <csv>]` — the dependency-impact neighborhood (memo §7.3): what the
/// finding rests on (upstream), what rests on it (downstream, the blast radius
/// if it moved), and the single points of failure on its support (the
/// minimal-evidence-cut). The finding resolves by id or assertion substring.
fn run_blast_radius(args: &[String]) {
    let mut frontier: Option<&str> = None;
    let mut finding: Option<&str> = None;
    let mut direction = BlastDirection::Both;
    let mut kinds: Vec<EdgeKind> = Vec::new();
    let mut i = 3; // after "atlas blast-radius"
    while i < args.len() {
        match args[i].as_str() {
            "--impact" => {
                direction = match args.get(i + 1).map(String::as_str) {
                    Some("up") | Some("upstream") => BlastDirection::Upstream,
                    Some("down") | Some("downstream") => BlastDirection::Downstream,
                    _ => BlastDirection::Both,
                };
                i += 2;
            }
            "--kinds" => {
                if let Some(csv) = args.get(i + 1) {
                    kinds = csv.split(',').filter_map(EdgeKind::parse).collect();
                }
                i += 2;
            }
            a if a.starts_with('-') => i += 1,
            a => {
                if frontier.is_none() {
                    frontier = Some(a);
                } else if finding.is_none() {
                    finding = Some(a);
                }
                i += 1;
            }
        }
    }
    let usage = "usage: vela atlas blast-radius <frontier> <finding> [--impact up|down|both] [--kinds <csv>]";
    let frontier = frontier.unwrap_or_else(|| fail(usage));
    let finding = finding.unwrap_or_else(|| fail(usage));
    let project = repo::load_from_path(Path::new(frontier))
        .unwrap_or_else(|e| fail(&format!("load {frontier}: {e}")));
    let graph = FrontierGraph::from_project(&project);
    let center = graph
        .find_node(finding)
        .unwrap_or_else(|| fail(&format!("no finding matching '{finding}' in {frontier}")));
    print_json(
        &graph
            .blast_radius_graded(&project, &center, &kinds, direction)
            .to_json(),
    );
}

/// `vela atlas decl-blast [--edges <jsonl>] [--decl <name>] [--top <N>] [--json]`
/// — the correction proof (memo §1.6) over a REAL premise graph. Loads the
/// Mathlib declaration-dependency graph (`data/mathlib/decl-edges.jsonl`,
/// `from --uses--> to`) as a `FrontierGraph` of `DependsOn` edges and reports the
/// downstream blast radius of retracting one declaration: every transitive
/// dependent that would need re-checking. Lean dependencies are CONJUNCTIVE
/// (a declaration requires every constant it uses), so the structural downstream
/// set IS the impacted set, exactly — there is no alternative route to survive
/// on, the distinction the κ-graded cascade draws on a verifier-gated frontier.
/// With no `--decl`, the highest-in-degree declaration (the highest-leverage
/// retraction) is chosen. This is the demonstration flat Erdős could not give
/// (no premise edges → 0 dependents).
fn run_decl_blast(args: &[String]) {
    use vela_protocol::frontier_graph::{BlastDirection, EdgeKind, FrontierGraph};

    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let edges_path =
        flag("--edges").unwrap_or_else(|| "data/mathlib/decl-edges-wide.jsonl".to_string());
    let top: usize = flag("--top").and_then(|s| s.parse().ok()).unwrap_or(15);
    let json_out = args.iter().any(|a| a == "--json");

    // Shared loader: accepts either the raw `decl-edges.jsonl` or the built
    // `decl-graph.v1.json` artifact, and applies the SAME noise-filter + dedup
    // so the retraction target is a real declaration, not a typeclass/core-type.
    let pairs = load_decl_edges(&edges_path)
        .unwrap_or_else(|e| fail(&format!("load decl edges {edges_path}: {e}")));
    if pairs.is_empty() {
        fail(&format!(
            "{edges_path}: no usable decl edges after noise filter"
        ));
    }
    let edges: Vec<(String, String, EdgeKind)> = pairs
        .into_iter()
        .map(|(from, to)| (from, to, EdgeKind::DependsOn))
        .collect();
    let graph = FrontierGraph::from_edges(edges);
    let ranked = graph.in_degree_ranked(&[EdgeKind::DependsOn]);

    let decl = flag("--decl")
        .and_then(|d| graph.find_node(&d))
        .or_else(|| ranked.first().map(|(id, _)| id.clone()))
        .unwrap_or_else(|| fail("no declaration to retract (empty graph)"));

    let blast = graph.blast_radius(&decl, &[EdgeKind::DependsOn], BlastDirection::Downstream);
    let in_deg = ranked
        .iter()
        .find(|(id, _)| *id == decl)
        .map(|(_, d)| *d)
        .unwrap_or(0);

    if json_out {
        print_json(&json!({
            "object": "vela.correction_blast.v1",
            "edges_source": edges_path,
            "nodes": graph.node_count(),
            "edges": graph.edge_count(),
            "retracted": decl,
            "direct_dependency_edges": in_deg,
            "impacted_total": blast.summary.downstream,
            "max_distance": blast.summary.max_downstream_distance,
            "model": "conjunctive premise graph (Lean dependencies): every transitive dependent is impacted; no alternative route survives",
            "impacted": blast.downstream.iter().take(top).map(|n| json!({
                "id": n.id, "distance": n.distance,
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    println!("correction blast-radius — retract `{decl}`");
    println!(
        "  premise graph: {} declarations, {} dependency edges ({})",
        graph.node_count(),
        graph.edge_count(),
        edges_path
    );
    println!("  retracted declaration is referenced by {in_deg} dependency edges");
    println!(
        "  => retracting it impacts {} downstream declarations (max depth {}), every one of which",
        blast.summary.downstream, blast.summary.max_downstream_distance
    );
    println!("     would need re-checking: Lean dependencies are conjunctive, so there is no");
    println!(
        "     alternative route to survive on. History is preserved (a correction mints a new root)."
    );
    if blast.summary.downstream == 0 {
        println!("  (this declaration is a leaf in the loaded slice: nothing depends on it here)");
    }
    for n in blast.downstream.iter().take(top) {
        println!("    - [d{}] {}", n.distance, n.id);
    }
    let shown = top.min(blast.downstream.len());
    if blast.downstream.len() > shown {
        println!("    … and {} more", blast.downstream.len() - shown);
    }
}

/// Slice policy v1 for the Mathlib decl-dependency graph. The raw "uses" edges
/// are dominated by typeclass plumbing + core types (Nat, DecidableEq, Finset,
/// the lattice/SMul instance chain) that EVERY declaration references, so the
/// highest-in-degree node — the "highest-leverage retraction" the correction
/// demo picks — is meaningless noise unless these are dropped. This denylist is
/// explicit + versioned so the slice is reviewable and the artifact is
/// deterministic. It is a LEGIBILITY filter over a projection, not a trust path.
const DECL_NOISE_EXACT: &[&str] = &[
    "Nat",
    "Int",
    "Eq",
    "Iff",
    "Exists",
    "And",
    "Or",
    "Not",
    "True",
    "False",
    "Bool",
    "Prop",
    "Finset",
    "List",
    "Set",
    "Multiset",
    "Subtype",
    "Sigma",
    "Prod",
    "Sum",
    "Option",
    "Quot",
    "Finset.card",
    "Finset.filter",
    "Finset.sum",
    "Finset.image",
    "Finset.instSetLike",
    "LE.le",
    "LT.lt",
    "GE.ge",
    "GT.gt",
    "Membership.mem",
    "SetLike.coe",
    "SetLike.instMembership",
    "HSMul.hSMul",
    "HMul.hMul",
    "HAdd.hAdd",
    "HSub.hSub",
    "HDiv.hDiv",
    "HPow.hPow",
    "Mul.mul",
    "Add.add",
    "OfNat.ofNat",
    "Zero.zero",
    "One.one",
    "Function.comp",
    // Order/algebra typeclasses: structural scaffolding, not theorems a correction
    // would retract. Dropping them pushes the highest-leverage retraction onto a
    // real declaration (a def/lemma) rather than a class.
    "LinearOrder",
    "PartialOrder",
    "Preorder",
    "Lattice",
    "SemilatticeInf",
    "SemilatticeSup",
    "DistribLattice",
    "CompleteLattice",
    "Order",
    "LinearOrder.toLattice",
    "AddMonoid",
    "Monoid",
    "AddCommMonoid",
    "CommMonoid",
    "AddGroup",
    "Group",
    "AddCommGroup",
    "CommGroup",
    "Ring",
    "CommRing",
    "Field",
    "Semiring",
    "CommSemiring",
    "Module",
    "Algebra",
    "Mul",
    "Add",
    "Zero",
    "One",
    "Neg",
    "Inv",
    "Sub",
    "Div",
    "Pow",
    "SMul",
    "Dvd",
    "Fintype",
    "DecidablePred",
    "Nonempty",
    "Finite",
    "Countable",
    "Encodable",
];

/// A target declaration is structural noise (a typeclass instance, a typeclass
/// coercion `X.toY`, a Decidable* witness, or a core type/relation) rather than
/// a theorem/lemma/def worth a correction-cascade node.
fn is_decl_noise(name: &str) -> bool {
    DECL_NOISE_EXACT.contains(&name)
        || name.starts_with("inst")
        || name.starts_with("Decidable")
        || name.contains(".to") // typeclass coercions, e.g. Lattice.toSemilatticeInf
}

/// Load decl-dependency edges from either the raw `decl-edges.jsonl`
/// (`{from,to,kind:"uses"}`) or the built `decl-graph.v1.json` artifact, applying
/// the noise filter + dedup + canonical sort. Deterministic: same input bytes →
/// same edge list. Returns `(from, to)` pairs (every edge is `from DependsOn to`).
pub(crate) fn load_decl_edges(path: &str) -> Result<Vec<(String, String)>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut set: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
    // The built artifact is a single JSON object with an `edges` array (it parses
    // as one Value); the raw source is one JSON object per line (it does not).
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(edges) = v.get("edges").and_then(|e| e.as_array())
    {
        for e in edges {
            if let (Some(from), Some(to)) = (
                e.get("from").and_then(|x| x.as_str()),
                e.get("to").and_then(|x| x.as_str()),
            ) && from != to
                && !is_decl_noise(to)
            {
                set.insert((from.to_string(), to.to_string()));
            }
        }
        return Ok(set.into_iter().collect());
    }
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(from), Some(to)) = (
            v.get("from").and_then(|x| x.as_str()),
            v.get("to").and_then(|x| x.as_str()),
        ) else {
            continue;
        };
        if from != to && !is_decl_noise(to) {
            set.insert((from.to_string(), to.to_string()));
        }
    }
    Ok(set.into_iter().collect())
}

/// `vela atlas decl-build [--in <jsonl>] [--out <json>]` — promote the raw
/// `decl-edges.jsonl` slice into a DETERMINISTIC, noise-filtered, deduped,
/// canonically-sorted premise-graph artifact (`decl-graph.v1.json`) that the
/// correction-cascade (`decl-blast`) runs over. Pins `source_sha256` +
/// `slice_policy` so the artifact is a pure function of (input, policy) and the
/// gate can re-derive it. Regenerable projection, NOT a reproduce-pinned
/// frontier — no wire-format change.
fn run_decl_build(args: &[String]) {
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let in_path = flag("--in").unwrap_or_else(|| "data/mathlib/decl-edges-wide.jsonl".to_string());
    let out_path = flag("--out").unwrap_or_else(|| "data/mathlib/decl-graph.v1.json".to_string());
    let json_out = args.iter().any(|a| a == "--json");

    let raw =
        std::fs::read_to_string(&in_path).unwrap_or_else(|e| fail(&format!("read {in_path}: {e}")));
    let source_sha256 = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(raw.as_bytes()))
    };
    let pairs = load_decl_edges(&in_path).unwrap_or_else(|e| fail(&e));
    let nodes: std::collections::BTreeSet<&str> = pairs
        .iter()
        .flat_map(|(a, b)| [a.as_str(), b.as_str()])
        .collect();

    let artifact = json!({
        "schema": "vela.decl-graph.v1",
        "source": in_path,
        "source_sha256": source_sha256,
        "slice_policy": "denylist v1: drop core types + typeclass instances/coercions (inst*, *.to*, Decidable*); dedup; canonical sort. from --uses--> to becomes from DependsOn to.",
        "stats": { "edges": pairs.len(), "nodes": nodes.len() },
        "edges": pairs.iter().map(|(f, t)| json!({ "from": f, "to": t })).collect::<Vec<_>>(),
    });
    let body = serde_json::to_string_pretty(&artifact).unwrap() + "\n";
    std::fs::write(&out_path, &body).unwrap_or_else(|e| fail(&format!("write {out_path}: {e}")));

    if json_out {
        print_json(&json!({
            "object": "vela.decl_graph_build.v1",
            "in": in_path, "out": out_path,
            "source_sha256": source_sha256,
            "edges": pairs.len(), "nodes": nodes.len(),
        }));
        return;
    }
    println!("built decl-graph artifact: {out_path}");
    println!("  source: {in_path} (sha256 {})", &source_sha256[..16]);
    println!(
        "  {} noise-filtered, deduped, canonically-sorted edges over {} declarations",
        pairs.len(),
        nodes.len()
    );
    println!(
        "  the correction cascade (`vela atlas decl-blast --edges {out_path}`) now retracts a"
    );
    println!(
        "  real declaration, not a typeclass instance. Deterministic: re-run yields identical bytes."
    );
}

/// The declared status of a finding parsed from its assertion text, normalized
/// to {open, solved, proved, disproved}. Handles the `declared status 'X'` form
/// (FC / formal corpus) and the Erdős prose form ("remains OPEN", "SOLVED", …).
fn declared_status(text: &str) -> Option<&'static str> {
    let lt = text.to_lowercase();
    let norm = |s: &str| -> Option<&'static str> {
        match s.trim() {
            "open" => Some("open"),
            "solved" => Some("solved"),
            "proved" => Some("proved"),
            "disproved" => Some("disproved"),
            _ => None,
        }
    };
    if let Some(i) = lt.find("declared status '") {
        let rest = &lt[i + "declared status '".len()..];
        if let Some(s) = rest.split('\'').next() {
            return norm(s);
        }
    }
    for (kw, st) in [
        ("disproved", "disproved"),
        ("remains open", "open"),
        ("is solved", "solved"),
        ("is proved", "proved"),
    ] {
        if lt.contains(kw) {
            return norm(st);
        }
    }
    None
}

/// `vela atlas gluing <frontier>... [--json]` — the presheaf-with-discord view
/// (plan B2 / THEORY §11.1). An atlas cell is the GLUING of local domain states
/// (the `vf_` members sharing a HardIdentity anchor across sources/contexts). The
/// cell glues into a coherent global frontier only where its members AGREE; a cell
/// whose members disagree is a GLUING OBSTRUCTION — the Belnap "both" (B) point,
/// already computed per cell as `AtlasCell.belnap`. There is no sheaf (unique
/// gluing fails, §11.1); the honest object is a presheaf with a discord `D_A`, and
/// `Frontier = supp(D_A)` is exactly the set of obstructed cells. Two discord
/// kinds: `verifier_discord` (belnap B — one member supported, another refuted, the
/// strong form) and `status_discord` (the weak form: a declared status conflict,
/// open vs resolved). Read-side projection over `atlas::project` + the bilattice;
/// never auto-adjudicated (a discord links a candidate `Contradiction`, a signal
/// for key-custody review).
fn run_gluing(args: &[String]) {
    use vela_protocol::bundle::bare_finding_id;
    use vela_protocol::contradiction::Contradiction;

    let frontiers: Vec<&str> = args
        .iter()
        .skip(3)
        .filter(|a| !a.starts_with('-'))
        .map(String::as_str)
        .collect();
    let json_out = args.iter().any(|a| a == "--json");
    if frontiers.is_empty() {
        fail("usage: vela atlas gluing <frontier> [<frontier> ...] [--json]");
    }
    let projects: Vec<_> = frontiers
        .iter()
        .map(|f| {
            repo::load_from_path(Path::new(f)).unwrap_or_else(|e| fail(&format!("load {f}: {e}")))
        })
        .collect();
    let mut status_of: std::collections::HashMap<String, &'static str> =
        std::collections::HashMap::new();
    for p in &projects {
        for f in &p.findings {
            if let Some(s) = declared_status(&f.assertion.text) {
                status_of.insert(f.id.clone(), s);
            }
        }
    }
    let refs: Vec<&_> = projects.iter().collect();
    let out = atlas::project(&refs);

    // The discord support: multi-member cells that fail to glue. A cell's joined
    // Belnap "B" is verifier-backed disagreement; a declared open-vs-resolved split
    // is the weaker declared-status discord.
    let mut obstructions: Vec<serde_json::Value> = Vec::new();
    let mut glued = 0usize;
    let mut multi = 0usize;
    for cell in &out.cells {
        if cell.members.len() < 2 {
            continue; // a singleton cell glues vacuously
        }
        multi += 1;
        // declared-status discord within the cell
        let mut by_status: std::collections::BTreeMap<&str, String> =
            std::collections::BTreeMap::new();
        for m in &cell.members {
            if let Some(s) = status_of.get(bare_finding_id(m)) {
                by_status
                    .entry(*s)
                    .or_insert_with(|| bare_finding_id(m).to_string());
            }
        }
        let open = by_status.get("open").cloned();
        let resolved = by_status
            .iter()
            .find(|(s, _)| **s != "open")
            .map(|(s, id)| (*s, id.clone()));
        let status_discord = matches!((&open, &resolved), (Some(_), Some(_)));
        let verifier_discord = cell.belnap == "B";
        if !verifier_discord && !status_discord {
            glued += 1;
            continue;
        }
        let anchor = cell
            .anchors
            .first()
            .map(|an| format!("{}:{}", an.namespace, an.id))
            .unwrap_or_else(|| "spine".to_string());
        let kind = if verifier_discord {
            "verifier_discord"
        } else {
            "status_discord"
        };
        // link a candidate contradiction (never auto-adjudicated)
        let (wa, wb) = match (&open, &resolved) {
            (Some(a), Some((_, b))) => (a.clone(), b.clone()),
            _ => {
                let mut it = cell.members.iter();
                (
                    it.next()
                        .map(|m| bare_finding_id(m).to_string())
                        .unwrap_or_default(),
                    it.next()
                        .map(|m| bare_finding_id(m).to_string())
                        .unwrap_or_default(),
                )
            }
        };
        let cx = Contradiction::candidate(&anchor, &wa, &wb, kind);
        obstructions.push(json!({
            "anchor": anchor,
            "label": cell.label.chars().take(80).collect::<String>(),
            "belnap": cell.belnap,
            "discord_kind": kind,
            "members": cell.members.iter().map(|m| bare_finding_id(m).to_string()).collect::<Vec<_>>(),
            "contradiction_id": cx.contradiction_id,
            "status": "candidate",
        }));
    }

    if json_out {
        print_json(&json!({
            "object": "vela.gluing_obstruction.v1",
            "frontiers": frontiers,
            "cells": out.cells.len(),
            "multi_member_cells": multi,
            "glued": glued,
            "discord_support": obstructions.len(),
            "doctrine": "Frontier = supp(D_A); a presheaf with discord, not a sheaf (no unique gluing, THEORY §11.1). Discords are candidates — never auto-adjudicated.",
            "obstructions": obstructions,
        }));
        return;
    }
    println!(
        "presheaf-with-discord — {} cells, {} multi-member, {} glue cleanly, {} in the discord support (Frontier = supp(D_A))",
        out.cells.len(),
        multi,
        glued,
        obstructions.len()
    );
    for o in &obstructions {
        println!(
            "  [{}] {} ({}) : {}",
            o["discord_kind"].as_str().unwrap_or(""),
            o["anchor"].as_str().unwrap_or(""),
            o["belnap"].as_str().unwrap_or(""),
            o["label"].as_str().unwrap_or("")
        );
    }
    if obstructions.is_empty() {
        println!("  (every multi-member cell glues — no discord; local states agree on overlaps)");
    } else {
        println!(
            "\ndiscords are candidates — never auto-adjudicated; resolution is key-custody review."
        );
    }
}

#[cfg(test)]
mod decl_graph_tests {
    use super::{is_decl_noise, load_decl_edges};

    #[test]
    fn noise_filter_drops_plumbing_keeps_lemmas() {
        // core types, typeclasses, instances, coercions, Decidable* -> noise.
        for n in [
            "Nat",
            "DecidableEq",
            "Finset",
            "LinearOrder",
            "Lattice",
            "instDistribLatticeOfLinearOrder",
            "SemilatticeInf.toPartialOrder",
        ] {
            assert!(is_decl_noise(n), "{n} should be noise");
        }
        // real declarations are kept.
        for n in [
            "Finset.univ",
            "Fintype.exists_card_fiber_lt_of_card_lt_mul",
            "Finset.exists_card_fiber_lt_of_card_lt_mul",
        ] {
            assert!(!is_decl_noise(n), "{n} should be kept");
        }
    }

    #[test]
    fn load_decl_edges_is_deterministic_and_deduped() {
        let dir = std::env::temp_dir().join(format!("vela_declgraph_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("edges.jsonl");
        // Two real edges (one duplicated) + two noise edges; whitespace-varied order.
        std::fs::write(
            &f,
            "{\"from\":\"A\",\"to\":\"B\",\"kind\":\"uses\"}\n\
             {\"from\":\"A\",\"to\":\"Nat\",\"kind\":\"uses\"}\n\
             {\"from\":\"A\",\"to\":\"B\",\"kind\":\"uses\"}\n\
             {\"from\":\"C\",\"to\":\"A\",\"kind\":\"uses\"}\n\
             {\"from\":\"C\",\"to\":\"DecidableEq\",\"kind\":\"uses\"}\n",
        )
        .unwrap();
        let p = f.to_str().unwrap();
        let a = load_decl_edges(p).unwrap();
        let b = load_decl_edges(p).unwrap();
        assert_eq!(a, b, "same input -> same output");
        // Noise dropped (Nat, DecidableEq), duplicate (A,B) collapsed, sorted.
        assert_eq!(
            a,
            vec![
                ("A".to_string(), "B".to_string()),
                ("C".to_string(), "A".to_string())
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
