pub(crate) fn require_artifact_reference_id(
    record: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    let valid_text =
        !value.trim().is_empty() && value == value.trim() && !value.chars().any(char::is_control);
    let is_content_hash = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid_text && (value.starts_with("va_") || is_content_hash) {
        return Ok(());
    }
    Err(format!(
        "{record} {field} must be a legacy va_ identifier or a full lowercase content hash"
    ))
}
