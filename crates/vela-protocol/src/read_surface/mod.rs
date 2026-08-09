//! Documents a Vela command answers with, as opposed to objects it signs.
//!
//! Everything under `objects/` builds canonical bytes and carries a signature.
//! Nothing here does. These are read surfaces: a consumer parses them, keys on
//! their `schema` field, and renders them. They are still wire contracts, and
//! they are contracts across a repository boundary rather than inside one, so
//! they are stated as types here for the same reason the signed objects are —
//! a shape restated in two places is a shape that drifts.

pub mod status;
