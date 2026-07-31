//! Experimental foreground host for routine evidence writes under one Attempt.
//!
//! The host is intentionally not an agent runner, scheduler, daemon, queue, or
//! authority surface. It accepts bounded NDJSON over inherited stdio and can
//! invoke only the two closed [`RoutineEvidenceController`] operations:
//! Submission registration and Verification import. Scientific Decisions,
//! policy changes, publication pushes, and arbitrary commands are absent.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner;
use crate::routine_evidence_controller::RoutineEvidenceController;

const HOST_RESPONSE_SCHEMA: &str = "vela.campaign-host-response.v1";
const CONTROL_FRAME_MAX_BYTES: usize = 64 * 1024;
const REQUEST_ID_MAX_BYTES: usize = 128;
const REQUEST_PATH_MAX_BYTES: usize = 4 * 1024;
const SUBMISSION_MAX_BYTES: u64 = 8 * 1024 * 1024;
const VERIFICATION_MAX_BYTES: u64 = 4 * 1024 * 1024;
const ORIGIN_MAX_BYTES: u64 = 2 * 1024 * 1024;
const REQUEST_CACHE_MAX_ENTRIES: usize = 256;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum HostRequest {
    RegisterSubmission { request_id: String, path: String },
    ImportVerification { request_id: String, path: String },
}

impl HostRequest {
    fn request_id(&self) -> &str {
        match self {
            Self::RegisterSubmission { request_id, .. }
            | Self::ImportVerification { request_id, .. } => request_id,
        }
    }

    fn operation(&self) -> &'static str {
        match self {
            Self::RegisterSubmission { .. } => "register_submission",
            Self::ImportVerification { .. } => "import_verification",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttemptBinding {
    attempt_id: String,
    frontier_id: String,
    path: PathBuf,
}

impl AttemptBinding {
    fn from_resolved(resolved: &crate::current_work::CurrentRoutineAttempt) -> Self {
        Self {
            attempt_id: resolved.attempt.attempt_id.clone(),
            frontier_id: resolved.attempt.frontier_id.clone(),
            path: resolved.path.clone(),
        }
    }
}

struct CampaignHostLock {
    file: fs::File,
}

impl CampaignHostLock {
    fn acquire(attempt_path: &Path) -> Result<Self, String> {
        let directory = attempt_path.parent().ok_or_else(|| {
            format!(
                "current Attempt path has no private directory: {}",
                attempt_path.display()
            )
        })?;
        let path = directory.join(".campaign-host.lock");
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(format!(
                "Campaign host lock must be a regular non-symlink file: {}",
                path.display()
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| format!("open Campaign host lock {}: {error}", path.display()))?;
        if !file
            .metadata()
            .map_err(|error| format!("inspect Campaign host lock {}: {error}", path.display()))?
            .is_file()
        {
            return Err(format!(
                "Campaign host lock is not a regular file: {}",
                path.display()
            ));
        }
        match file.try_lock() {
            Ok(()) => Ok(Self { file }),
            Err(fs::TryLockError::WouldBlock) => Err(format!(
                "another Campaign host already owns Attempt {}",
                attempt_path.display()
            )),
            Err(fs::TryLockError::Error(error)) => {
                Err(format!("lock Campaign host {}: {error}", path.display()))
            }
        }
    }
}

impl Drop for CampaignHostLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

struct CampaignHost {
    frontier: PathBuf,
    inbox: PathBuf,
    inbox_identity: same_file::Handle,
    attempt: AttemptBinding,
    _lock: CampaignHostLock,
    controller: RoutineEvidenceController,
}

impl CampaignHost {
    fn open(frontier: &Path, attempt_id: &str, inbox: &Path) -> Result<Self, String> {
        let frontier = frontier
            .canonicalize()
            .map_err(|error| format!("resolve current Frontier {}: {error}", frontier.display()))?;
        let inbox_metadata = fs::symlink_metadata(inbox)
            .map_err(|error| format!("inspect Campaign inbox {}: {error}", inbox.display()))?;
        if inbox_metadata.file_type().is_symlink() || !inbox_metadata.is_dir() {
            return Err(format!(
                "Campaign inbox must be a real directory: {}",
                inbox.display()
            ));
        }
        let inbox = inbox
            .canonicalize()
            .map_err(|error| format!("resolve Campaign inbox {}: {error}", inbox.display()))?;
        let inbox_identity = same_file::Handle::from_path(&inbox)
            .map_err(|error| format!("identify Campaign inbox {}: {error}", inbox.display()))?;

        let resolved =
            crate::current_work::resolve_verification_attempt(&frontier, Some(attempt_id))?
                .ok_or_else(|| format!("current Attempt {attempt_id} is unavailable"))?;
        let attempt = AttemptBinding::from_resolved(&resolved);
        let lock = CampaignHostLock::acquire(&attempt.path)?;
        drop(resolved);

        let repository = crate::current_repository::verify_current_repository_at(&frontier, true)?;
        if repository.frontier_id != attempt.frontier_id {
            return Err(format!(
                "current Attempt {} belongs to Frontier {}, not {}",
                attempt.attempt_id, attempt.frontier_id, repository.frontier_id
            ));
        }
        let origin_bytes = crate::bounded_file::read_bounded_frontier_file(
            &frontier,
            Path::new(".vela/origin.json"),
            ORIGIN_MAX_BYTES,
            "current repository origin",
        )
        .map_err(|error| error.to_string())?;
        let origin = RepositoryOriginV1::parse(&origin_bytes)?;
        if origin.canonical_bytes()? != origin_bytes {
            return Err("current repository origin is not canonical JSON".into());
        }
        let authority =
            crate::cli::load_current_repository_authority(&frontier, &repository, &origin)?;
        let (key_id, public_key) = crate::workflow::active_repository_signing_key(&authority)?;
        let signer = SshAgentRepositoryAuthoritySigner::from_environment(key_id, &public_key)?;
        let controller = RoutineEvidenceController::new(Box::new(signer));

        Ok(Self {
            frontier,
            inbox,
            inbox_identity,
            attempt,
            _lock: lock,
            controller,
        })
    }

    fn read_inbox_file(
        &self,
        relative: &str,
        max_bytes: u64,
        label: &str,
    ) -> Result<(PathBuf, Vec<u8>), String> {
        self.verify_inbox_identity()?;
        let relative = validate_request_path(relative)?;
        let bytes = crate::bounded_file::read_bounded_frontier_file(
            &self.inbox,
            &relative,
            max_bytes,
            label,
        )
        .map_err(|error| error.to_string())?;
        self.verify_inbox_identity()?;
        Ok((self.inbox.join(relative), bytes))
    }

    fn verify_inbox_identity(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.inbox).map_err(|error| {
            format!("reinspect Campaign inbox {}: {error}", self.inbox.display())
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "Campaign inbox changed from a real directory: {}",
                self.inbox.display()
            ));
        }
        let observed = same_file::Handle::from_path(&self.inbox).map_err(|error| {
            format!(
                "reidentify Campaign inbox {}: {error}",
                self.inbox.display()
            )
        })?;
        if observed != self.inbox_identity {
            return Err(format!(
                "Campaign inbox identity changed while the host was active: {}",
                self.inbox.display()
            ));
        }
        Ok(())
    }

    fn handle(&mut self, request: &HostRequest) -> Result<Value, String> {
        match request {
            HostRequest::RegisterSubmission { path, .. } => {
                let (submission_path, bytes) =
                    self.read_inbox_file(path, SUBMISSION_MAX_BYTES, "Campaign Submission")?;
                let submission = SubmissionV1::parse(&bytes)?;
                require_content_addressed_transport(&submission)?;
                self.verify_inbox_identity()?;
                let bundle_root = submission_path.parent().ok_or_else(|| {
                    format!(
                        "Campaign Submission has no bundle directory: {}",
                        submission_path.display()
                    )
                })?;
                let outcome = self.controller.register_submission(
                    &self.frontier,
                    &submission,
                    &submission.provenance.producer,
                    &self.attempt.attempt_id,
                    Some(bundle_root),
                    false,
                )?;
                if outcome.accepted_event_delta != 0 || outcome.accepted_state_changed {
                    return Err(
                        "routine Submission registration unexpectedly changed accepted state"
                            .into(),
                    );
                }
                serde_json::to_value(outcome)
                    .map_err(|error| format!("serialize Submission outcome: {error}"))
            }
            HostRequest::ImportVerification { path, .. } => {
                let (_, bytes) = self.read_inbox_file(
                    path,
                    VERIFICATION_MAX_BYTES,
                    "Campaign Verification Record",
                )?;
                let record = VerificationRecordV1::parse(&bytes)?;
                let outcome = self.controller.import_verification(
                    &self.frontier,
                    &record,
                    &record.verifier,
                    &self.attempt.attempt_id,
                    false,
                )?;
                if outcome.accepted_event_delta != 0 {
                    return Err(
                        "routine Verification import unexpectedly changed accepted state".into(),
                    );
                }
                serde_json::to_value(outcome)
                    .map_err(|error| format!("serialize Verification outcome: {error}"))
            }
        }
    }
}

fn require_content_addressed_transport(submission: &SubmissionV1) -> Result<(), String> {
    for (index, artifact) in submission.artifacts.iter().enumerate() {
        let digest = artifact
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("Submission artifact {index} digest is not sha256"))?;
        let expected = format!("records/artifacts/sha256/{digest}");
        if artifact.path != expected {
            return Err(format!(
                "Campaign Submission artifact {index} must use the content-addressed transport path {expected}"
            ));
        }
    }
    Ok(())
}

enum Frame {
    Eof,
    Data(Vec<u8>),
    Oversized,
}

fn read_frame(reader: &mut impl BufRead) -> io::Result<Frame> {
    let mut frame = Vec::new();
    let mut saw_input = false;
    let mut oversized = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if !saw_input {
                return Ok(Frame::Eof);
            }
            break;
        }
        saw_input = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        if !oversized {
            let remaining = CONTROL_FRAME_MAX_BYTES
                .saturating_add(1)
                .saturating_sub(frame.len());
            let copied = content_len.min(remaining);
            frame.extend_from_slice(&available[..copied]);
            if copied < content_len || frame.len() > CONTROL_FRAME_MAX_BYTES {
                oversized = true;
            }
        }
        let consumed = newline.map_or(available.len(), |index| index + 1);
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    if oversized {
        return Ok(Frame::Oversized);
    }
    if frame.last() == Some(&b'\r') {
        frame.pop();
    }
    Ok(Frame::Data(frame))
}

fn validate_request_id(request_id: &str) -> Result<(), String> {
    if request_id.is_empty()
        || request_id.len() > REQUEST_ID_MAX_BYTES
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(format!(
            "request_id must contain 1..={REQUEST_ID_MAX_BYTES} ASCII letters, digits, '.', '_', ':', or '-'"
        ));
    }
    Ok(())
}

fn validate_request_path(path: &str) -> Result<PathBuf, String> {
    if path.is_empty()
        || path.len() > REQUEST_PATH_MAX_BYTES
        || path.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(format!(
            "Campaign object path must contain 1..={REQUEST_PATH_MAX_BYTES} non-control bytes"
        ));
    }
    let path = PathBuf::from(path);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(
            "Campaign object path must be normalized and relative to the approved inbox".into(),
        );
    }
    Ok(path)
}

fn response(
    request_id: Option<&str>,
    operation: Option<&str>,
    result: Result<Value, String>,
) -> Value {
    match result {
        Ok(result) => json!({
            "schema": HOST_RESPONSE_SCHEMA,
            "ok": true,
            "request_id": request_id,
            "operation": operation,
            "result": result,
        }),
        Err(message) => json!({
            "schema": HOST_RESPONSE_SCHEMA,
            "ok": false,
            "request_id": request_id,
            "operation": operation,
            "error": {
                "kind": "domain",
                "message": crate::cli::safe_text::multiline(&message),
            },
        }),
    }
}

fn write_response(writer: &mut impl Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| format!("serialize Campaign host response: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|()| writer.flush())
        .map_err(|error| format!("write Campaign host response: {error}"))
}

fn serve(
    host: &mut CampaignHost,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<(), String> {
    let mut requests = BTreeMap::<String, (HostRequest, Option<Value>)>::new();
    loop {
        let frame =
            read_frame(reader).map_err(|error| format!("read Campaign host request: {error}"))?;
        let Frame::Data(bytes) = frame else {
            match frame {
                Frame::Eof => return Ok(()),
                Frame::Oversized => {
                    write_response(
                        writer,
                        &response(
                            None,
                            None,
                            Err(format!(
                                "Campaign host request exceeds the {CONTROL_FRAME_MAX_BYTES}-byte limit"
                            )),
                        ),
                    )?;
                    continue;
                }
                Frame::Data(_) => unreachable!(),
            }
        };
        let request = match serde_json::from_slice::<HostRequest>(&bytes) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    writer,
                    &response(
                        None,
                        None,
                        Err(format!("parse Campaign host request: {error}")),
                    ),
                )?;
                continue;
            }
        };
        let request_id = request.request_id().to_string();
        let operation = request.operation();
        if let Err(error) = validate_request_id(&request_id) {
            write_response(writer, &response(None, Some(operation), Err(error)))?;
            continue;
        }
        if let Some((bound, cached)) = requests.get(&request_id) {
            if bound != &request {
                write_response(
                    writer,
                    &response(
                        Some(&request_id),
                        Some(operation),
                        Err("request_id is already bound to a different Campaign request".into()),
                    ),
                )?;
            } else if let Some(cached) = cached {
                write_response(writer, cached)?;
            } else {
                let value = response(Some(&request_id), Some(operation), host.handle(&request));
                if value["ok"] == true {
                    requests
                        .get_mut(&request_id)
                        .expect("request binding exists")
                        .1 = Some(value.clone());
                }
                write_response(writer, &value)?;
            }
            continue;
        }
        if requests.len() >= REQUEST_CACHE_MAX_ENTRIES {
            write_response(
                writer,
                &response(
                    Some(&request_id),
                    Some(operation),
                    Err(format!(
                        "Campaign host request cache reached its {REQUEST_CACHE_MAX_ENTRIES}-entry bound; restart the foreground host"
                    )),
                ),
            )?;
            continue;
        }
        requests.insert(request_id.clone(), (request.clone(), None));
        let value = response(Some(&request_id), Some(operation), host.handle(&request));
        if value["ok"] == true {
            requests
                .get_mut(&request_id)
                .expect("request binding exists")
                .1 = Some(value.clone());
        }
        write_response(writer, &value)?;
    }
}

pub(crate) fn cmd_host(frontier: &Path, attempt: &str, inbox: &Path) {
    let mut host = match CampaignHost::open(frontier, attempt, inbox) {
        Ok(host) => host,
        Err(error) => {
            let value = response(None, None, Err(error));
            let _ = write_response(&mut io::stdout().lock(), &value);
            std::process::exit(1);
        }
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    if let Err(error) = serve(&mut host, &mut reader, &mut writer) {
        let value = response(None, None, Err(error));
        let _ = write_response(&mut writer, &value);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn request_surface_has_only_two_mutating_operations() {
        let submission: HostRequest = serde_json::from_str(
            r#"{"operation":"register_submission","request_id":"r:2","path":"run/submission.json"}"#,
        )
        .unwrap();
        let verification: HostRequest = serde_json::from_str(
            r#"{"operation":"import_verification","request_id":"r:3","path":"run/verification.json"}"#,
        )
        .unwrap();
        assert_eq!(submission.operation(), "register_submission");
        assert_eq!(verification.operation(), "import_verification");
        for forbidden in [
            "status",
            "stop",
            "accept",
            "reject",
            "cancel",
            "review",
            "policy_change",
            "push",
            "run_command",
        ] {
            let request = format!(r#"{{"operation":"{forbidden}","request_id":"r:5"}}"#);
            assert!(serde_json::from_str::<HostRequest>(&request).is_err());
        }
    }

    #[test]
    fn request_rejects_unknown_fields_and_unsafe_paths() {
        assert!(
            serde_json::from_str::<HostRequest>(
                r#"{"operation":"register_submission","request_id":"r:1","path":"submission.json","decision":"accept"}"#
            )
            .is_err()
        );
        assert!(validate_request_path("../submission.json").is_err());
        assert!(validate_request_path("/tmp/submission.json").is_err());
        assert!(validate_request_path("./submission.json").is_err());
        assert!(validate_request_path("run/submission.json").is_ok());
    }

    #[test]
    fn oversized_frame_is_discarded_without_desynchronizing_the_stream() {
        let mut input = vec![b'x'; CONTROL_FRAME_MAX_BYTES + 1];
        input.extend_from_slice(
            b"\n{\"operation\":\"import_verification\",\"request_id\":\"after\",\"path\":\"verification.json\"}\n",
        );
        let mut reader = Cursor::new(input);
        assert!(matches!(read_frame(&mut reader).unwrap(), Frame::Oversized));
        let Frame::Data(second) = read_frame(&mut reader).unwrap() else {
            panic!("second frame must remain readable");
        };
        let request: HostRequest = serde_json::from_slice(&second).unwrap();
        assert_eq!(request.request_id(), "after");
        assert!(matches!(read_frame(&mut reader).unwrap(), Frame::Eof));
    }

    #[test]
    fn response_is_one_compact_json_line() {
        let mut output = Vec::new();
        write_response(
            &mut output,
            &response(
                Some("r:1"),
                Some("register_submission"),
                Ok(json!({"state": "pending_review"})),
            ),
        )
        .unwrap();
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert!(!output[..output.len() - 1].contains(&b'\n'));
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["request_id"], "r:1");
    }
}
