use std::io::Read;

use crate::{
    Approval, Custody, EnrollmentRequest, ProtectionMode, SessionRecord, SessionState,
    SignerRequest, approve_and_sign, enroll,
};
use clap::{Parser, Subcommand};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use zeroize::Zeroize;

const KEYRING_SERVICE: &str = "science.vela.signer.v1";
const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

#[derive(Parser)]
#[command(
    name = "vela-signer",
    version,
    about = "One-shot Vela human decision signer"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read one closed request from stdin, show one decision card, sign, exit.
    Approve,
    /// Read one closed enrollment request from stdin, install and verify custody.
    Enroll,
}

struct SystemApproval;

impl Approval for SystemApproval {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    fn ensure_session(&self, request: &SignerRequest) -> Result<(), String> {
        let now = chrono::Utc::now();
        if load_session_record().is_some_and(|record| {
            record.state(
                &request.reviewer_actor,
                &request.reviewer_public_key,
                &request.provider,
                protection_mode_name(request.protection_mode),
                &request.helper_sha256,
                now,
            ) == SessionState::Active
        }) {
            return Ok(());
        }
        platform_reauthenticate(&format!(
            "Unlock Vela decisions for reviewer {}",
            request.reviewer_actor
        ))?;
        save_session_record(&SessionRecord::new(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            protection_mode_name(request.protection_mode),
            &request.helper_sha256,
            chrono::Utc::now(),
        ))
    }

    fn approve(&self, request: &SignerRequest) -> Result<bool, String> {
        let title = format!("Vela: {} one proposal", capitalize(&request.action));
        let description = format!(
            "{} proposal\n{}\n\nFrontier\n{}\n\nReason\n{}\n\nProposal root\n{}\n\nDecision Plan\n{}\n\nGate\n{}\n\nCustody\n{} / {} ({:?})",
            capitalize(&request.action),
            request.proposal_id,
            request.frontier_id,
            request.reason,
            request.proposal_root,
            request.decision_plan_root,
            request.gate_state,
            request.provider,
            request.protection_grade,
            request.protection_mode,
        );
        let result = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(description)
            .set_buttons(MessageButtons::YesNo)
            .show();
        Ok(matches!(result, MessageDialogResult::Yes))
    }

    fn record_session_use(
        &self,
        request: &SignerRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        let mut record = load_session_record()
            .ok_or_else(|| "signer session disappeared before completion".to_string())?;
        let now = chrono::Utc::now();
        record.touch(now)?;
        record.sign(key)?;
        if record.state(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            protection_mode_name(request.protection_mode),
            &request.helper_sha256,
            now,
        ) != SessionState::Active
        {
            return Err("signer session binding changed before completion".to_string());
        }
        save_session_record(&record)
    }

    fn reauthenticate(&self, request: &SignerRequest) -> Result<(), String> {
        platform_reauthenticate(&format!(
            "{} {} at Decision Plan {}",
            capitalize(&request.action),
            request.proposal_id,
            request
                .decision_plan_root
                .chars()
                .take(23)
                .collect::<String>()
        ))
    }

    fn approve_enrollment(&self, request: &EnrollmentRequest) -> Result<bool, String> {
        let description = format!(
            "Protect this Vela human identity?\n\nActor\n{}\n\nPublic key\n{}\n\nProvider\n{} ({:?})\n\nAfter protected readback succeeds, Vela will remove the plaintext source:\n{}",
            request.actor,
            request.public_key,
            request.provider,
            request.protection_mode,
            request.source_path,
        );
        let result = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title("Vela: Protect human signing identity")
            .set_description(description)
            .set_buttons(MessageButtons::YesNo)
            .show();
        Ok(matches!(result, MessageDialogResult::Yes))
    }

    fn reauthenticate_enrollment(&self, request: &EnrollmentRequest) -> Result<(), String> {
        platform_reauthenticate(&format!("Protect Vela identity {}", request.actor))?;
        save_session_record(&SessionRecord::new(
            &request.actor,
            &request.public_key,
            &request.provider,
            protection_mode_name(request.protection_mode),
            &request.helper_sha256,
            chrono::Utc::now(),
        ))
    }

    fn record_enrollment_session(
        &self,
        request: &EnrollmentRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        let mut record = load_session_record()
            .ok_or_else(|| "signer enrollment session disappeared".to_string())?;
        let now = chrono::Utc::now();
        record.touch(now)?;
        record.sign(key)?;
        if record.state(
            &request.actor,
            &request.public_key,
            &request.provider,
            protection_mode_name(request.protection_mode),
            &request.helper_sha256,
            now,
        ) != SessionState::Active
        {
            return Err("signer enrollment session binding changed".to_string());
        }
        save_session_record(&record)
    }
}

fn protection_mode_name(mode: ProtectionMode) -> &'static str {
    match mode {
        ProtectionMode::Session => "session",
        ProtectionMode::Always => "always",
    }
}

struct SystemCustody {
    protection_mode: ProtectionMode,
}

impl SystemCustody {
    fn new(protection_mode: ProtectionMode) -> Self {
        Self { protection_mode }
    }
}

fn session_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| "cannot resolve the user home for signer session state".to_string())?;
    Ok(std::path::PathBuf::from(home)
        .join(".vela")
        .join("signer-session.json"))
}

fn load_session_record() -> Option<SessionRecord> {
    let bytes = std::fs::read(session_path().ok()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_session_record(record: &SessionRecord) -> Result<(), String> {
    let path = session_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "signer session path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create signer session directory: {error}"))?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let bytes =
        serde_json::to_vec(record).map_err(|error| format!("serialize signer session: {error}"))?;
    std::fs::write(&temporary, bytes).map_err(|error| format!("write signer session: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("protect signer session: {error}"))?;
    }
    std::fs::rename(&temporary, &path).map_err(|error| format!("install signer session: {error}"))
}

impl Custody for SystemCustody {
    fn provider(&self) -> &str {
        "os_store"
    }

    fn provider_session(&self) -> Result<String, String> {
        if self.protection_mode == ProtectionMode::Always {
            return Ok("per_use".to_string());
        }
        load_session_record()
            .map(|record| record.session_id)
            .ok_or_else(|| "signer session record is missing after approval".to_string())
    }

    fn protection_grade(&self) -> &str {
        "user_session"
    }

    fn load_seed(&self, actor: &str, public_key: &str) -> Result<[u8; 32], String> {
        let account = format!("{actor}:{public_key}");
        let entry = keyring::Entry::new(KEYRING_SERVICE, &account)
            .map_err(|error| format!("open OS credential entry: {error}"))?;
        let mut bytes = entry
            .get_secret()
            .map_err(|error| format!("read OS-protected Vela key: {error}"))?;
        let seed = bytes
            .as_slice()
            .try_into()
            .map_err(|_| "OS-protected Vela key is not a 32-byte Ed25519 seed".to_string())?;
        bytes.zeroize();
        Ok(seed)
    }

    fn store_seed(&self, actor: &str, public_key: &str, seed: &[u8; 32]) -> Result<(), String> {
        let account = format!("{actor}:{public_key}");
        let entry = keyring::Entry::new(KEYRING_SERVICE, &account)
            .map_err(|error| format!("open OS credential entry: {error}"))?;
        entry
            .set_secret(seed)
            .map_err(|error| format!("store OS-protected Vela key: {error}"))
    }

    fn delete_seed(&self, actor: &str, public_key: &str) -> Result<(), String> {
        let account = format!("{actor}:{public_key}");
        let entry = keyring::Entry::new(KEYRING_SERVICE, &account)
            .map_err(|error| format!("open OS credential entry: {error}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("delete OS-protected Vela key: {error}")),
        }
    }
}

pub fn main_entry() {
    if let Err(error) = run() {
        eprintln!("vela-signer: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    harden_process()?;
    let cli = Cli::parse();
    match cli.command {
        Command::Approve => approve_once(),
        Command::Enroll => enroll_once(),
    }
}

#[cfg(unix)]
fn harden_process() -> Result<(), String> {
    let current = rustix::process::getrlimit(rustix::process::Resource::Core);
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: current.maximum,
        },
    )
    .map_err(|error| format!("disable signer core dumps: {error}"))?;
    #[cfg(target_os = "linux")]
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|error| format!("disable signer process attachment: {error}"))?;
    Ok(())
}

#[cfg(not(unix))]
fn harden_process() -> Result<(), String> {
    Ok(())
}

fn approve_once() -> Result<(), String> {
    let mut bytes = read_request_bytes()?;
    let request: SignerRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse closed signer request: {error}"))?;
    bytes.zeroize();
    let helper_path =
        std::env::current_exe().map_err(|error| format!("resolve running helper path: {error}"))?;
    let response = approve_and_sign(
        &request,
        &SystemApproval,
        &SystemCustody::new(request.protection_mode),
        &helper_path,
        chrono::Utc::now(),
    )?;
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("serialize signer response: {error}"))?
    );
    Ok(())
}

fn enroll_once() -> Result<(), String> {
    let mut bytes = read_request_bytes()?;
    let request: EnrollmentRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse closed enrollment request: {error}"))?;
    bytes.zeroize();
    let helper_path =
        std::env::current_exe().map_err(|error| format!("resolve running helper path: {error}"))?;
    let response = enroll(
        &request,
        &SystemApproval,
        &SystemCustody::new(request.protection_mode),
        &helper_path,
        chrono::Utc::now(),
    )?;
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("serialize enrollment response: {error}"))?
    );
    Ok(())
}

fn read_request_bytes() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read helper request: {error}"))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        bytes.zeroize();
        return Err("helper request exceeds 1 MiB".to_string());
    }
    Ok(bytes)
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn platform_reauthenticate(reason: &str) -> Result<(), String> {
    use block2::RcBlock;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};
    use std::sync::mpsc;
    use std::time::Duration;

    let context = unsafe { LAContext::new() };
    unsafe {
        context
            .canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthentication)
            .map_err(|error| format!("macOS reauthentication is unavailable: {error}"))?;
    }
    let localized_reason = NSString::from_str(reason);
    let (tx, rx) = mpsc::sync_channel(1);
    let reply = RcBlock::new(move |success: objc2::runtime::Bool, _error: *mut NSError| {
        let _ = tx.send(success.as_bool());
    });
    unsafe {
        context.evaluatePolicy_localizedReason_reply(
            LAPolicy::DeviceOwnerAuthentication,
            &localized_reason,
            &reply,
        );
    }
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("platform reauthentication was cancelled or failed".to_string()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            unsafe { context.invalidate() };
            Err("platform reauthentication timed out".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("platform reauthentication ended without a result".to_string())
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn platform_reauthenticate(reason: &str) -> Result<(), String> {
    use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows::core::HSTRING;

    let window = unsafe { GetConsoleWindow() };
    if window.0.is_null() {
        return Err(
            "Windows Hello needs a foreground console window for this signer request".to_string(),
        );
    }
    let interop = windows::core::factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
        .map_err(|error| format!("open Windows Hello desktop interop: {error}"))?;
    let result: UserConsentVerificationResult = unsafe {
        interop.RequestVerificationForWindowAsync::<
            windows_future::IAsyncOperation<UserConsentVerificationResult>,
        >(window, &HSTRING::from(reason))
    }
    .map_err(|error| format!("start Windows Hello verification: {error}"))?
    .join()
    .map_err(|error| format!("complete Windows Hello verification: {error}"))?;
    if result == UserConsentVerificationResult::Verified {
        Ok(())
    } else {
        Err(format!(
            "Windows Hello verification did not approve the request: {result:?}"
        ))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_reauthenticate(_reason: &str) -> Result<(), String> {
    let process = linux_process_subject()?;
    let status = std::process::Command::new("pkcheck")
        .args([
            "--action-id",
            "science.vela.signer.authenticate",
            "--process",
            &process,
            "--allow-user-interaction",
        ])
        .status()
        .map_err(|error| {
            format!(
                "start polkit reauthentication; install the Vela signer policy and pkcheck: {error}"
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "polkit did not authorize Vela signer use (pkcheck status {status})"
        ))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_process_subject() -> Result<String, String> {
    let stat = std::fs::read_to_string("/proc/self/stat")
        .map_err(|error| format!("read Linux process start time: {error}"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| "Linux process stat omitted the command boundary".to_string())?;
    let start_time = stat[close + 1..]
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| "Linux process stat omitted the start time".to_string())?;
    Ok(format!(
        "{},{},{}",
        std::process::id(),
        start_time,
        rustix::process::geteuid().as_raw()
    ))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn platform_reauthenticate(_reason: &str) -> Result<(), String> {
    Err("always mode is unsupported on this platform".to_string())
}
