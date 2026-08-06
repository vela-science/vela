//! Shared CLI serialization and failure helpers.

use serde::Serialize;

use crate::ui::ErrorKind;

/// Fail without claiming to know why.
///
/// `Domain` is the honest default for the interior of a verb: by the time a
/// `Result<_, String>` reaches the dispatch arm, the string has erased which of
/// several unrelated failures produced it, and guessing a kind there would put
/// a wrong exit code on the contract. Sites that CAN name their kind use
/// [`fail_kind`].
pub(crate) fn fail(message: &str) -> ! {
    crate::ui::fail_with(ErrorKind::Domain, message, None)
}

pub(crate) fn fail_return<T>(message: &str) -> T {
    fail(message)
}

/// Fail with the kind the call site can actually name, so the exit code carries
/// the contract `ui.rs` documents rather than collapsing to 1.
///
/// Use this only where the kind follows from the failure itself — the object
/// was looked up and is not here, the flag value was parsed and is not legal —
/// never from the verb the failure happens to sit in.
pub(crate) fn fail_kind(kind: ErrorKind, message: &str) -> ! {
    crate::ui::fail_with(kind, message, None)
}

pub(crate) fn fail_kind_return<T>(kind: ErrorKind, message: &str) -> T {
    fail_kind(kind, message)
}

pub(crate) fn print_json<T: Serialize + ?Sized>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize json output")
    );
}
