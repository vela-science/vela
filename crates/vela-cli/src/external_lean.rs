//! Installed external-Lean execution boundary.
//!
//! The Python core is embedded in the release artifact.  It is also imported
//! by the campaign adapter, making the OS sandbox policy one implementation
//! rather than a checkout-time copy.  Fetching and pinning happen before this
//! boundary; only the pinned execution request enters it.

use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::receipt_v1::{
    ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput, ReceiptV1,
};

// Keep the complete installed verifier bundle inside this publishable crate.
// Reaching into the parent campaign checkout makes an otherwise valid public
// clone and crates.io package fail at compile time.
const DRIVER: &[u8] = include_bytes!("../resources/external_lean_sandbox.py");
const ONRAMP: &[u8] = include_bytes!("../resources/external_lean_verifier.py");
const RECEIPT_CORE_PY: &[u8] = include_bytes!("../resources/vela_receipt_v1.py");
const RECEIPT_HARNESS_PY: &[u8] = include_bytes!("../resources/receipt_v1.py");
const RECEIPT_JSON_PY: &[u8] = include_bytes!("../resources/receipt_json.py");
const MAX_RESULT_BYTES: usize = 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_ONRAMP_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ROOT_ENTRIES: usize = 250_000;
const MAX_ROOT_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const ISOLATED_PYTHON_BOOTSTRAP: &str = "import runpy,sys; scripts=sys.argv[1]; driver=sys.argv[2]; sys.path.insert(0,scripts); sys.argv=[driver,*sys.argv[3:]]; runpy.run_path(driver,run_name='__main__')";

#[derive(Debug)]
pub(crate) struct ExternalLeanPull<'a> {
    pub repo_url: &'a str,
    pub commit: &'a str,
    pub declaration: &'a str,
    pub source_path: Option<&'a str>,
    pub output_root: &'a Path,
    pub runtime_root: &'a Path,
}

#[derive(Debug)]
struct ReceiptArtifact {
    path: String,
    kind: String,
    sha256: String,
}

#[derive(Debug)]
struct ReceiptContext {
    emitted_at: String,
    event_log_root: String,
    base_path: String,
    policy_ref: String,
    task_contract_root: Option<String>,
}

fn validate_bounded_root(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "external-Lean root must be absolute: {}",
            path.display()
        ));
    }
    let root_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect external-Lean root {}: {error}", path.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(format!(
            "external-Lean root must be a real directory: {}",
            path.display()
        ));
    }
    let mut stack = vec![path.to_path_buf()];
    let mut entries = 0usize;
    let mut bytes = 0u64;
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory)
            .map_err(|error| format!("enumerate {}: {error}", directory.display()))?
        {
            let entry =
                entry.map_err(|error| format!("enumerate {}: {error}", directory.display()))?;
            entries = entries.saturating_add(1);
            if entries > MAX_ROOT_ENTRIES {
                return Err(format!(
                    "external-Lean root exceeds {MAX_ROOT_ENTRIES} entries"
                ));
            }
            let child = entry.path();
            if child.as_os_str().len() > 4096 {
                return Err("external-Lean root contains an overlong path".to_string());
            }
            let metadata = std::fs::symlink_metadata(&child)
                .map_err(|error| format!("inspect {}: {error}", child.display()))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                stack.push(child);
            } else if metadata.is_file() {
                bytes = bytes.saturating_add(metadata.len());
                if bytes > MAX_ROOT_BYTES {
                    return Err(format!("external-Lean root exceeds {MAX_ROOT_BYTES} bytes"));
                }
            } else {
                return Err(format!(
                    "external-Lean root contains a special file: {}",
                    child.display()
                ));
            }
        }
    }
    Ok(())
}

fn request_root_paths(request: &Value) -> Result<Vec<PathBuf>, String> {
    let object = request
        .as_object()
        .ok_or_else(|| "external-Lean request must be an object".to_string())?;
    let mut roots = Vec::new();
    for key in ["read_roots", "execution_copy_roots"] {
        let Some(value) = object.get(key) else {
            continue;
        };
        let values = value
            .as_array()
            .ok_or_else(|| format!("external-Lean {key} must be an array"))?;
        if values.len() > 64 {
            return Err(format!("external-Lean {key} exceeds 64 roots"));
        }
        for value in values {
            let value = value
                .as_str()
                .ok_or_else(|| format!("external-Lean {key} contains a non-string root"))?;
            roots.push(PathBuf::from(value));
        }
    }
    let write_root = object
        .get("write_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "external-Lean write_root must be a string".to_string())?;
    roots.push(PathBuf::from(write_root));
    Ok(roots)
}

fn write_embedded(path: &Path, bytes: &[u8], executable: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("embedded path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create embedded directory {}: {error}", parent.display()))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if executable { 0o500 } else { 0o400 });
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("materialize embedded {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write embedded {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("persist embedded {}: {error}", path.display()))
}

fn validate_pull_arguments(request: &ExternalLeanPull<'_>) -> Result<(), String> {
    for (label, value) in [
        ("repository URL", request.repo_url),
        ("commit", request.commit),
        ("declaration", request.declaration),
    ] {
        if value.is_empty()
            || value.len() > MAX_ONRAMP_ARGUMENT_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(format!(
                "external-Lean {label} must be non-empty, bounded, and free of control characters"
            ));
        }
    }
    if let Some(source_path) = request.source_path
        && (source_path.is_empty()
            || source_path.len() > MAX_ONRAMP_ARGUMENT_BYTES
            || source_path.chars().any(char::is_control))
    {
        return Err(
            "external-Lean source path must be non-empty, bounded, and free of control characters"
                .to_string(),
        );
    }
    validate_bounded_root(request.output_root)?;
    validate_bounded_root(request.runtime_root)
}

fn parse_single_result(output: std::process::Output, label: &str) -> Result<Value, String> {
    if output.stdout.len() > MAX_RESULT_BYTES || output.stderr.len() > MAX_RESULT_BYTES {
        return Err(format!("{label} exceeded its output boundary"));
    }
    let mut stream = serde_json::Deserializer::from_slice(&output.stdout).into_iter::<Value>();
    let result = stream
        .next()
        .ok_or_else(|| {
            format!(
                "{label} returned no JSON object: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        })?
        .map_err(|error| format!("{label} returned invalid JSON: {error}"))?;
    if stream.next().is_some() {
        return Err(format!("{label} returned more than one JSON value"));
    }
    if !result.is_object() {
        return Err(format!("{label} result must be a JSON object"));
    }
    let semantic_ok = result
        .get("ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{label} result omitted boolean ok"))?;
    if output.status.success() != semantic_ok {
        return Err(format!(
            "{label} exit/result disagreement: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(result)
}

fn python_path() -> OsString {
    if let Some(explicit) = std::env::var_os("VELA_EXTERNAL_LEAN_PYTHON") {
        return explicit;
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join("python3");
            if candidate.is_file()
                && let Ok(canonical) = candidate.canonicalize()
            {
                return canonical.into_os_string();
            }
        }
    }
    OsString::from("/usr/bin/python3")
}

fn child_path() -> OsString {
    let mut paths = vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")];
    if let Some(elan_home) = std::env::var_os("ELAN_HOME") {
        paths.push(PathBuf::from(elan_home).join("bin"));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".elan").join("bin"));
    }
    std::env::join_paths(paths).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

/// Run the complete commit-pinned producer path from bytes embedded in the
/// installed binary. The checkout script is never discovered or executed.
/// This function produces evidence only; Receipt construction and landing are
/// separate Rust calls below.
pub(crate) fn run_external_reproduction(request: &ExternalLeanPull<'_>) -> Result<Value, String> {
    validate_pull_arguments(request)?;
    let installation = tempfile::Builder::new()
        .prefix("vela-installed-external-lean-")
        .tempdir()
        .map_err(|error| format!("create installed external-Lean bundle: {error}"))?;
    let install_root = installation.path();
    let scripts = install_root.join("scripts");
    let driver_path = scripts.join("external_lean_verifier.py");
    write_embedded(&driver_path, ONRAMP, true)?;
    write_embedded(&scripts.join("vela_receipt_v1.py"), RECEIPT_CORE_PY, false)?;
    write_embedded(&scripts.join("receipt_v1.py"), RECEIPT_HARNESS_PY, false)?;
    write_embedded(&scripts.join("receipt_json.py"), RECEIPT_JSON_PY, false)?;
    write_embedded(
        &install_root
            .join("vendor")
            .join("vela")
            .join("crates")
            .join("vela-cli")
            .join("resources")
            .join("external_lean_sandbox.py"),
        DRIVER,
        true,
    )?;

    let mut command = Command::new(python_path());
    command
        .arg("-I")
        .arg("-B")
        .arg("-c")
        .arg(ISOLATED_PYTHON_BOOTSTRAP)
        .arg(&scripts)
        .arg(&driver_path)
        .arg("--repo-url")
        .arg(request.repo_url)
        .arg("--commit")
        .arg(request.commit)
        .arg("--declaration")
        .arg(request.declaration)
        .arg("--output-root")
        .arg(request.output_root)
        .arg("--installed-result-only")
        .arg("--json")
        .env_clear()
        .env("PATH", child_path())
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONNOUSERSITE", "1")
        .env("VELA_EXTERNAL_LEAN_RUNTIME_ROOT", request.runtime_root);
    if let Some(source_path) = request.source_path {
        command.arg("--source-path").arg(source_path);
    }
    for name in ["HOME", "ELAN_HOME"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if std::env::var("VELA_ALLOW_LOCAL_EXTERNAL_LEAN_FIXTURE").as_deref() == Ok("1") {
        // Deliberate conformance-only escape hatch: local commit-pinned fixture
        // repositories still execute through the identical installed driver,
        // bounded copy, and OS sandbox. No other inherited producer setting is
        // forwarded into the isolated child.
        command.env("VELA_ALLOW_LOCAL_EXTERNAL_LEAN_FIXTURE", "1");
    }
    let output = command
        .output()
        .map_err(|error| format!("could not start installed external-Lean producer: {error}"))?;
    parse_single_result(output, "installed external-Lean producer")
}

/// SHA-256 identity of the sandbox driver embedded in this binary.
pub fn embedded_driver_root() -> String {
    format!("sha256:{:x}", Sha256::digest(DRIVER))
}

/// Run one already-pinned sandbox request through the embedded driver.
///
/// This function performs no source fetch, no proposal mutation, and no
/// landing.  A caller may feed the bounded result into the private Receipt
/// builder and then the shared land service.
pub fn run_sandbox_request(request: &Value) -> Result<Value, String> {
    for root in request_root_paths(request)? {
        validate_bounded_root(&root)?;
    }
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("could not create external-Lean driver directory: {error}"))?;
    let driver_path = temporary.path().join("external_lean_sandbox.py");
    let request_path = temporary.path().join("request.json");

    let mut driver_options = OpenOptions::new();
    driver_options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        driver_options.mode(0o500);
    }
    let mut driver = driver_options
        .open(&driver_path)
        .map_err(|error| format!("could not materialize embedded external-Lean driver: {error}"))?;
    driver
        .write_all(DRIVER)
        .map_err(|error| format!("could not write embedded external-Lean driver: {error}"))?;
    driver
        .sync_all()
        .map_err(|error| format!("could not persist embedded external-Lean driver: {error}"))?;

    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| format!("could not encode external-Lean request: {error}"))?;
    if request_bytes.len() > MAX_REQUEST_BYTES {
        return Err("external-Lean request exceeded its byte boundary".to_string());
    }
    let mut request_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&request_path)
        .map_err(|error| format!("could not create external-Lean request: {error}"))?;
    request_file
        .write_all(&request_bytes)
        .map_err(|error| format!("could not write external-Lean request: {error}"))?;
    request_file
        .sync_all()
        .map_err(|error| format!("could not persist external-Lean request: {error}"))?;

    let output = Command::new(python_path())
        .arg("-I")
        .arg("-B")
        .arg(&driver_path)
        .arg("--request")
        .arg(&request_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONNOUSERSITE", "1")
        .output()
        .map_err(|error| format!("could not start embedded external-Lean driver: {error}"))?;

    // The Python boundary uses exit 1 for every bounded refusal. Preserve the
    // structured result so callers can distinguish sandbox absence, an unsafe
    // request, a resource limit, and a Lean failure without parsing stderr.
    parse_single_result(output, "embedded external-Lean driver")
}

fn runtime_root() -> Result<PathBuf, String> {
    let root = if let Some(explicit) = std::env::var_os("VELA_EXTERNAL_LEAN_CACHE") {
        let path = PathBuf::from(explicit);
        if !path.is_absolute() {
            return Err("VELA_EXTERNAL_LEAN_CACHE must be an absolute path".to_string());
        }
        path
    } else {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is required to locate the external-Lean cache".to_string())?;
        #[cfg(target_os = "macos")]
        let path = home
            .join("Library")
            .join("Caches")
            .join("Vela")
            .join("external-lean");
        #[cfg(not(target_os = "macos"))]
        let path = home.join(".cache").join("vela").join("external-lean");
        path
    };
    fs::create_dir_all(&root)
        .map_err(|error| format!("create external-Lean cache {}: {error}", root.display()))?;
    validate_bounded_root(&root)?;
    root.canonicalize()
        .map_err(|error| format!("resolve external-Lean cache {}: {error}", root.display()))
}

fn absolute_output_path(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "--out must name a receipt file".to_string())?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("create receipt parent {}: {error}", parent.display()))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| format!("resolve receipt parent {}: {error}", parent.display()))?;
    Ok(parent.join(file_name))
}

fn checked_file_bytes(path: &Path, root: &Path, expected_sha256: &str) -> Result<Vec<u8>, String> {
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("external-Lean artifact reported an invalid SHA-256".to_string());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect external-Lean artifact {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "external-Lean artifact must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > MAX_ARTIFACT_BYTES {
        return Err(format!(
            "external-Lean artifact {} exceeds {MAX_ARTIFACT_BYTES} bytes",
            path.display()
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("resolve artifact root {}: {error}", root.display()))?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("resolve artifact {}: {error}", path.display()))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "external-Lean artifact escapes its parent-owned root: {}",
            path.display()
        ));
    }
    let bytes = fs::read(&canonical).map_err(|error| {
        format!(
            "read external-Lean artifact {}: {error}",
            canonical.display()
        )
    })?;
    if hex::encode(Sha256::digest(&bytes)) != expected_sha256 {
        return Err(format!(
            "external-Lean artifact digest disagrees with bytes: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn write_evidence_file(path: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err(format!(
            "external-Lean evidence exceeds {MAX_ARTIFACT_BYTES} bytes"
        ));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o400);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(bytes)
                .map_err(|error| format!("write evidence {}: {error}", path.display()))?;
            file.sync_all()
                .map_err(|error| format!("persist evidence {}: {error}", path.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|inspect| {
                format!("inspect existing evidence {}: {inspect}", path.display())
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "existing evidence is not a regular non-symlink file: {}",
                    path.display()
                ));
            }
            let existing = fs::read(path)
                .map_err(|read| format!("read existing evidence {}: {read}", path.display()))?;
            if existing != bytes {
                return Err(format!(
                    "existing evidence bytes differ: {}",
                    path.display()
                ));
            }
        }
        Err(error) => return Err(format!("create evidence {}: {error}", path.display())),
    }
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn artifact_reference<'a>(response: &'a Value, name: &str) -> Option<(&'a str, &'a str)> {
    let reference = response.get(name)?.as_object()?;
    Some((
        reference.get("path")?.as_str()?,
        reference.get("sha256")?.as_str()?,
    ))
}

fn retain_receipt_artifacts(
    response: &Value,
    staging_root: &Path,
    destination: &Path,
    descriptor_base: &Path,
) -> Result<Vec<ReceiptArtifact>, String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "create external-Lean evidence directory {}: {error}",
            destination.display()
        )
    })?;
    let destination_metadata = fs::symlink_metadata(destination).map_err(|error| {
        format!(
            "inspect external-Lean evidence directory {}: {error}",
            destination.display()
        )
    })?;
    if destination_metadata.file_type().is_symlink() || !destination_metadata.is_dir() {
        return Err(format!(
            "external-Lean evidence directory must be a real directory: {}",
            destination.display()
        ));
    }

    let mut artifacts = Vec::new();
    let typed_result = response
        .get("result")
        .ok_or_else(|| "external-Lean response omitted its typed result".to_string())?;
    let mut result_bytes = serde_json::to_vec_pretty(typed_result)
        .map_err(|error| format!("encode external-Lean typed result: {error}"))?;
    result_bytes.push(b'\n');
    let result_path = destination.join("reproduction-result.json");
    let result_sha256 = write_evidence_file(&result_path, &result_bytes)?;
    artifacts.push(ReceiptArtifact {
        path: result_path
            .strip_prefix(descriptor_base)
            .map_err(|_| "retained result is outside its descriptor base".to_string())?
            .to_string_lossy()
            .replace('\\', "/"),
        kind: "external_lean_reproduction_result".to_string(),
        sha256: result_sha256,
    });

    for (field, file_name, kind) in [
        (
            "source_manifest",
            "source-manifest.json",
            "external_lean_source_manifest",
        ),
        (
            "verifier_log",
            "reproduction-log.json",
            "external_lean_reproduction_log",
        ),
    ] {
        let Some((source, expected_sha256)) = artifact_reference(response, field) else {
            continue;
        };
        let bytes = checked_file_bytes(Path::new(source), staging_root, expected_sha256)?;
        let path = destination.join(file_name);
        let sha256 = write_evidence_file(&path, &bytes)?;
        artifacts.push(ReceiptArtifact {
            path: path
                .strip_prefix(descriptor_base)
                .map_err(|_| "retained artifact is outside its descriptor base".to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
            kind: kind.to_string(),
            sha256,
        });
    }
    Ok(artifacts)
}

fn active_work_context(
    frontier: &Path,
    target: &str,
    actor: &str,
) -> Result<ReceiptContext, String> {
    let session = crate::workflow::resolve_work_session(frontier, actor, Some(target))?;
    let policy_ref = vela_protocol::acceptance_policy::load_active_policy(frontier)?
        .map(|verified| verified.policy.id)
        .unwrap_or_else(|| "urn:vela:policy:none".to_string());
    Ok(ReceiptContext {
        emitted_at: session.record.created_at,
        event_log_root: session.record.base_event_log_root,
        base_path: session.relative_dir,
        policy_ref,
        task_contract_root: Some(session.record.task_contract_root),
    })
}

fn detached_context() -> ReceiptContext {
    ReceiptContext {
        emitted_at: Utc::now().to_rfc3339(),
        event_log_root: format!("sha256:{}", vela_protocol::events::event_log_hash(&[])),
        base_path: ".".to_string(),
        policy_ref: "urn:vela:policy:none".to_string(),
        task_contract_root: None,
    }
}

fn result_claim(result: &Value) -> Result<(String, String, String, String), String> {
    if result.get("schema").and_then(Value::as_str)
        != Some("vela.external_lean_reproduction_result.v1")
        || result.get("ok").and_then(Value::as_bool) != Some(true)
    {
        return Err("external-Lean typed result has an unsupported schema or status".to_string());
    }
    let identity = result
        .get("identity")
        .and_then(Value::as_object)
        .ok_or_else(|| "external-Lean typed result has no identity".to_string())?;
    let declaration = identity
        .get("declaration")
        .and_then(Value::as_str)
        .ok_or_else(|| "external-Lean typed result has no declaration".to_string())?;
    let commit = identity
        .get("source_commit")
        .and_then(Value::as_str)
        .ok_or_else(|| "external-Lean typed result has no source commit".to_string())?;
    let verdict = result
        .get("verdict")
        .and_then(Value::as_str)
        .ok_or_else(|| "external-Lean typed result has no verdict".to_string())?;
    let (claim, claim_type, caveat, outcome) = match verdict {
        "reproduced" => (
            format!(
                "Vela reproduced Lean declaration {declaration} at commit {commit} in the frozen environment."
            ),
            "theoretical",
            "This is a reproduction verdict, not acceptance of the declaration as frontier truth.",
            "pass",
        ),
        "reproduction_failed" => (
            format!(
                "Vela could not reproduce Lean declaration {declaration} at commit {commit} in the frozen environment."
            ),
            "negative",
            "Build failure does not establish that the mathematical statement is false.",
            "fail",
        ),
        "dirty_axioms" => (
            format!(
                "Lean declaration {declaration} at commit {commit} builds outside Vela's clean axiom set."
            ),
            "negative",
            "Dirty axioms block reproduction; they are not a truth verdict on the statement.",
            "fail",
        ),
        "contradicted" => (
            format!(
                "A frozen check contradicted the scoped statement for Lean declaration {declaration} at commit {commit}."
            ),
            "contradiction",
            "Contradiction is scoped to the frozen counter-witness and authorizes no acceptance.",
            "fail",
        ),
        "skipped_with_reason" => (
            format!(
                "Vela skipped reproduction of Lean declaration {declaration} at commit {commit} with a typed reason."
            ),
            "negative",
            "A skipped reproduction is neither a build result nor a truth or quality verdict.",
            "skipped",
        ),
        other => return Err(format!("unsupported external-Lean verdict: {other}")),
    };
    Ok((
        claim,
        claim_type.to_string(),
        caveat.to_string(),
        outcome.to_string(),
    ))
}

fn build_receipt(
    response: &Value,
    artifacts: &[ReceiptArtifact],
    actor: &str,
    context: ReceiptContext,
) -> Result<ReceiptV1, String> {
    if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
        return Err("external-Lean receipt production requires an agent:/ci: actor".to_string());
    }
    let result = response
        .get("result")
        .ok_or_else(|| "external-Lean response omitted its typed result".to_string())?;
    let (claim, claim_type, caveat, outcome) = result_claim(result)?;
    let artifact_inputs = artifacts
        .iter()
        .map(|artifact| {
            ArtifactInput::new(
                artifact.path.clone(),
                artifact.kind.clone(),
                Some(artifact.sha256.clone()),
                None,
            )
            .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let verifier_run = ProducerReportedRun::producer_reported(
        "verifier.lean_external_declaration.v1".to_string(),
        outcome,
    )
    .map_err(|error| error.to_string())?;
    let operation_preimage = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.external-lean-receipt.internal.v1",
        "actor": actor,
        "claim": claim,
        "artifacts": artifacts.iter().map(|artifact| json!({
            "path": artifact.path,
            "kind": artifact.kind,
            "sha256": artifact.sha256,
        })).collect::<Vec<_>>(),
        "base_path": context.base_path,
        "event_log_root": context.event_log_root,
        "policy_ref": context.policy_ref,
        "task_contract_root": context.task_contract_root,
    }))?;
    let operation_id =
        crate::operation_journal::operation_id("reproduce-external", &operation_preimage);
    let key = vela_edge::vela_agent_mcp::agent_signing_key(Some(actor))?;
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: context.emitted_at.clone(),
        },
        &key,
    )?;
    let mut input = ReceiptInput::new(
        claim,
        claim_type,
        "exact".to_string(),
        artifact_inputs,
        vec![
            caveat,
            "This is producer-reported evidence only; landing, policy routing, and human acceptance remain separate."
                .to_string(),
        ],
        vec![verifier_run],
        actor.to_string(),
        context.emitted_at,
        context.event_log_root,
        context.base_path,
        operation_id,
        context.policy_ref,
    )
    .map_err(|error| error.to_string())?;
    if let Some(task_contract_root) = context.task_contract_root {
        input = input
            .with_task_contract_root(task_contract_root)
            .map_err(|error| error.to_string())?;
    }
    ReceiptBuilder::build(input, &identity).map_err(|error| error.to_string())
}

fn write_receipt(path: &Path, receipt: &ReceiptV1) -> Result<(), String> {
    let bytes = receipt
        .canonical_bytes()
        .map_err(|error| error.to_string())?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("create receipt {}: {error}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("write receipt {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .map_err(|error| format!("finish receipt {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("persist receipt {}: {error}", path.display()))
}

fn emit_failure(json_output: bool, message: &str) -> ! {
    if json_output {
        crate::cli::print_json(&json!({
            "ok": false,
            "command": "reproduce-external",
            "error": message,
        }));
    } else {
        eprintln!("{} {message}", vela_protocol::cli_style::err_prefix());
    }
    std::process::exit(1)
}

fn landed_receipt_value(
    target: &str,
    task_contract_root: &str,
    outcome: crate::workflow::LandOutcome,
) -> Value {
    let (route, detail) = outcome.route.summary();
    let accepted_event_delta = outcome.accepted_event_delta();
    json!({
        "content_address": outcome.receipt_root,
        "work_target": target,
        "task_contract_root": task_contract_root,
        "record_id": outcome.record_id,
        "proposal_id": outcome.proposal_id,
        "finding_id": outcome.finding_id,
        "accepted_event_count_before": outcome.accepted_event_count_before,
        "accepted_event_count_after": outcome.accepted_event_count_after,
        "accepted_event_delta": accepted_event_delta,
        "landed": true,
        "route": route,
        "detail": detail,
        "operation_id": outcome.operation_id,
        "publication": outcome.publication,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_reproduce_external(
    repo_url: String,
    commit: String,
    declaration: String,
    source_path: Option<String>,
    out: Option<PathBuf>,
    land_work: Option<String>,
    frontier: Option<PathBuf>,
    actor: Option<String>,
    json_output: bool,
) {
    crate::ui::set_mode("reproduce-external", json_output);
    let staging = tempfile::Builder::new()
        .prefix("vela-external-lean-result-")
        .tempdir()
        .unwrap_or_else(|error| {
            emit_failure(json_output, &format!("create result staging: {error}"))
        });
    let runtime = runtime_root().unwrap_or_else(|error| emit_failure(json_output, &error));

    if land_work.is_none() && frontier.is_some() {
        emit_failure(json_output, "--frontier requires --land-work");
    }
    let resolved_frontier = land_work
        .as_ref()
        .map(|_| crate::ui::resolve_frontier(frontier));
    let resolved_actor = if out.is_some() || land_work.is_some() {
        let actor = crate::cli_identity::resolve_actor(actor.as_deref());
        if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
            emit_failure(
                json_output,
                "receipt production requires an agent:/ci: producer identity",
            );
        }
        Some(actor)
    } else if actor.is_some() {
        emit_failure(json_output, "--as requires --out or --land-work");
    } else {
        None
    };

    let request = ExternalLeanPull {
        repo_url: &repo_url,
        commit: &commit,
        declaration: &declaration,
        source_path: source_path.as_deref(),
        output_root: staging.path(),
        runtime_root: &runtime,
    };
    let mut response = run_external_reproduction(&request)
        .unwrap_or_else(|error| emit_failure(json_output, &error));
    response["installed_onramp"] = json!({
        "embedded": true,
        "checkout_discovery": false,
        "content_root": embedded_onramp_root(),
        "artifact_retention": if out.is_some() || land_work.is_some() {"receipt_bound"} else {"ephemeral"},
    });
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        if json_output {
            crate::cli::print_json(&response);
        } else {
            eprintln!(
                "{} {}",
                vela_protocol::cli_style::err_prefix(),
                response
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("external-Lean producer failed")
            );
        }
        std::process::exit(1);
    }

    if let Some(out) = out {
        let out =
            absolute_output_path(&out).unwrap_or_else(|error| emit_failure(json_output, &error));
        let file_name = out
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("receipt.json");
        let artifact_dir = out
            .parent()
            .expect("absolute receipt has parent")
            .join(format!(".{file_name}.artifacts"));
        let artifacts = retain_receipt_artifacts(
            &response,
            staging.path(),
            &artifact_dir,
            out.parent().expect("absolute receipt has parent"),
        )
        .unwrap_or_else(|error| emit_failure(json_output, &error));
        let receipt = build_receipt(
            &response,
            &artifacts,
            resolved_actor
                .as_deref()
                .expect("actor resolved for receipt"),
            detached_context(),
        )
        .unwrap_or_else(|error| emit_failure(json_output, &error));
        write_receipt(&out, &receipt).unwrap_or_else(|error| emit_failure(json_output, &error));
        response["receipt"] = json!({
            "path": out,
            "content_address": receipt.canonical_root().unwrap_or_default(),
            "landed": false,
            "acceptance_status": "emitted",
        });
    } else if let Some(target) = land_work {
        let frontier = resolved_frontier
            .as_deref()
            .expect("frontier resolved for land-work");
        let context = active_work_context(
            frontier,
            &target,
            resolved_actor
                .as_deref()
                .expect("actor resolved for land-work"),
        )
        .unwrap_or_else(|error| emit_failure(json_output, &error));
        let task_contract_root = context
            .task_contract_root
            .clone()
            .expect("active work context has a task contract root");
        let result_key = response
            .get("result_key")
            .and_then(Value::as_str)
            .unwrap_or("unknown-result");
        let artifact_dir = frontier
            .join(&context.base_path)
            .join("external-lean")
            .join(result_key);
        let artifacts =
            retain_receipt_artifacts(&response, staging.path(), &artifact_dir, frontier)
                .unwrap_or_else(|error| emit_failure(json_output, &error));
        let receipt = build_receipt(
            &response,
            &artifacts,
            resolved_actor
                .as_deref()
                .expect("actor resolved for receipt"),
            context,
        )
        .unwrap_or_else(|error| emit_failure(json_output, &error));
        let outcome = crate::workflow::land(
            frontier,
            &receipt,
            resolved_actor
                .as_deref()
                .expect("actor resolved for receipt"),
            false,
        )
        .unwrap_or_else(|error| emit_failure(json_output, &error));
        response["receipt"] = landed_receipt_value(&target, &task_contract_root, outcome);
    }

    if json_output {
        crate::cli::print_json(&response);
    } else {
        let verdict = response
            .get("verdict")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        println!("{verdict}");
        if let Some(receipt) = response.get("receipt").filter(|value| !value.is_null()) {
            println!(
                "receipt: {}",
                receipt
                    .get("content_address")
                    .and_then(Value::as_str)
                    .unwrap_or("emitted")
            );
        }
    }
}

/// Content identity of the complete installed producer bundle.
pub fn embedded_onramp_root() -> String {
    let mut digest = Sha256::new();
    for bytes in [
        ONRAMP,
        RECEIPT_CORE_PY,
        RECEIPT_HARNESS_PY,
        RECEIPT_JSON_PY,
        DRIVER,
    ] {
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::git_publish::{PublicationOutcome, PublicationState};
    use crate::workflow::{LandOutcome, LandRoute};

    #[test]
    fn embedded_driver_has_stable_content_identity() {
        let root = embedded_driver_root();
        assert!(root.starts_with("sha256:"));
        assert_eq!(root.len(), 71);
        assert!(DRIVER.starts_with(b"#!/usr/bin/env python3\n"));
        assert_eq!(embedded_onramp_root().len(), 71);
        assert!(ONRAMP.starts_with(b"#!/usr/bin/env python3\n"));
    }

    #[test]
    fn embedded_onramp_sources_are_crate_local_package_resources() {
        let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        for (name, embedded) in [
            ("external_lean_sandbox.py", DRIVER),
            ("external_lean_verifier.py", ONRAMP),
            ("vela_receipt_v1.py", RECEIPT_CORE_PY),
            ("receipt_v1.py", RECEIPT_HARNESS_PY),
            ("receipt_json.py", RECEIPT_JSON_PY),
        ] {
            let packaged = fs::read(resources.join(name))
                .unwrap_or_else(|error| panic!("missing packaged resource {name}: {error}"));
            assert_eq!(packaged, embedded, "packaged resource {name} drifted");
        }
    }

    #[test]
    fn landed_external_work_reports_target_base_and_exact_event_delta() {
        let value = landed_receipt_value(
            "external-lean:fixture",
            "sha256:task-contract",
            LandOutcome {
                operation_id: "vop_fixture".to_string(),
                receipt_root: "sha256:receipt".to_string(),
                record_id: "vrc_fixture".to_string(),
                proposal_id: "vpr_fixture".to_string(),
                finding_id: "vf_fixture".to_string(),
                accepted_event_count_before: Some(12),
                accepted_event_count_after: Some(12),
                route: LandRoute::Deferred {
                    reasons: vec!["human decision required".to_string()],
                },
                publication: PublicationOutcome {
                    state: PublicationState::CommittedLocal {
                        commit: "0123456789abcdef".to_string(),
                    },
                    recovery_command: Some("git push origin producer/fixture".to_string()),
                },
            },
        );

        assert_eq!(value["work_target"], "external-lean:fixture");
        assert_eq!(value["task_contract_root"], "sha256:task-contract");
        assert_eq!(value["accepted_event_count_before"], 12);
        assert_eq!(value["accepted_event_count_after"], 12);
        assert_eq!(value["accepted_event_delta"], 0);
        assert_eq!(value["route"], "deferred");
        assert_eq!(value["publication"]["state"], "committed_local");
    }
}
