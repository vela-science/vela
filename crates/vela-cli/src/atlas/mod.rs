//! Cross-frontier atlas projection + source adapters.
//! Re-exported flat (`crate::cli_*`) at the crate root; file organization only.

// The CLI no longer invokes these adapters directly: they are retained as the
// pure conversion seam for Receipt producers, not as canonical-state writers.
#[allow(dead_code)]
pub mod atlas_adapters;
pub mod cli_atlas;
