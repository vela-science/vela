pub(crate) fn require_artifact_reference_id(
    record: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    let valid_text =
        !value.trim().is_empty() && value == value.trim() && !value.chars().any(char::is_control);
    let is_content_hash = crate::shape::is_lower_hex_64(value);
    if valid_text && is_content_hash {
        return Ok(());
    }
    Err(format!(
        "{record} {field} must be a full lowercase content hash"
    ))
}
