//! Shared CLI serialization and failure helpers.

use serde::Serialize;

pub(crate) fn fail(message: &str) -> ! {
    crate::ui::fail_with(crate::ui::ErrorKind::Domain, message, None)
}

pub(crate) fn fail_return<T>(message: &str) -> T {
    fail(message)
}

pub(crate) fn print_json<T: Serialize + ?Sized>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize json output")
    );
}
