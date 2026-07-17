//! Deterministic declaration-dependency parsing used by read-only server projections.\n\n/// Slice policy v1 for the Mathlib decl-dependency graph. The raw "uses" edges
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
