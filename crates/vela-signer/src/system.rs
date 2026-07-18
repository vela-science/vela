use std::io::Read;

use crate::{
    Approval, Custody, EnrollmentRequest, PolicySignerRequest, ProtectionMode, RebindRequest,
    SessionRecord, SessionState, SignerRequest, approve_and_sign, approve_and_sign_policy, enroll,
    rebind,
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
    /// Read one closed policy request from stdin, show one exact card, sign, exit.
    ApprovePolicy,
    /// Read one closed enrollment request from stdin, install and verify custody.
    Enroll,
    /// Rebind an existing protected identity to this exact helper binary.
    Rebind,
}

struct SystemApproval;

impl Approval for SystemApproval {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    fn ensure_session(&self, request: &SignerRequest) -> Result<(), String> {
        ensure_bound_session(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            "open a signer session",
            &format!(
                "Unlock Vela decisions for reviewer {}",
                request.reviewer_actor
            ),
        )
    }

    fn approve(&self, request: &SignerRequest) -> Result<bool, String> {
        require_user_interaction("show a decision card")?;
        let card = decision_card(request);
        // Keep cancellation as the platform default. A stray Return key or a
        // GUI backend that cannot distinguish a closed window from its first
        // button must fail closed rather than authorize scientific state.
        let result = MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title(&card.title)
            .set_description(&card.description)
            .set_buttons(MessageButtons::OkCancelCustom(
                "Cancel".to_string(),
                card.action_label.clone(),
            ))
            .show();
        Ok(card_approved(result, &card.action_label))
    }

    fn record_session_use(
        &self,
        request: &SignerRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        record_bound_session_use(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            key,
        )
    }

    fn reauthenticate(&self, request: &SignerRequest) -> Result<(), String> {
        require_user_interaction("reauthenticate a protected decision")?;
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

    fn ensure_policy_session(&self, request: &PolicySignerRequest) -> Result<(), String> {
        ensure_bound_session(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            "open a protected policy session",
            &format!(
                "Unlock Vela policy decisions for {}",
                request.reviewer_actor
            ),
        )
    }

    fn approve_policy(&self, request: &PolicySignerRequest) -> Result<bool, String> {
        require_user_interaction("show a policy decision card")?;
        let card = policy_decision_card(request);
        let result = MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title(&card.title)
            .set_description(&card.description)
            .set_buttons(MessageButtons::OkCancelCustom(
                "Cancel".to_string(),
                card.action_label.clone(),
            ))
            .show();
        Ok(card_approved(result, &card.action_label))
    }

    fn record_policy_session_use(
        &self,
        request: &PolicySignerRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        record_bound_session_use(
            &request.reviewer_actor,
            &request.reviewer_public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            key,
        )
    }

    fn reauthenticate_policy(&self, request: &PolicySignerRequest) -> Result<(), String> {
        require_user_interaction("reauthenticate a protected policy decision")?;
        platform_reauthenticate(&format!(
            "{} policy {} at Decision Plan {}",
            capitalize(request.action.as_str()),
            request.selected_policy_id,
            short_root(&request.decision_plan_root)
        ))
    }

    fn reauthenticate_enrollment(&self, request: &EnrollmentRequest) -> Result<(), String> {
        require_user_interaction("authenticate protected enrollment")?;
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

    fn reauthenticate_rebind(&self, request: &RebindRequest) -> Result<(), String> {
        require_user_interaction("authenticate a signer update")?;
        if request.purpose == crate::RebindPurpose::EnrollmentRecovery {
            platform_reauthenticate(&format!(
                "Resume protected Vela identity enrollment for {}",
                request.actor
            ))?;
        } else {
            platform_reauthenticate(&format!(
                "Authorize Vela signer update for {}: Vela {} to {}; helper {} to {}; mode {:?} to {:?}",
                request.actor,
                short_root(&request.previous_vela_binary_sha256),
                short_root(&request.vela_binary_sha256),
                short_root(&request.previous_helper_sha256),
                short_root(&request.helper_sha256),
                request.previous_protection_mode,
                request.protection_mode,
            ))?;
        }
        if request.protection_mode == ProtectionMode::Session {
            save_session_record(&SessionRecord::new(
                &request.actor,
                &request.public_key,
                &request.provider,
                protection_mode_name(request.protection_mode),
                &request.helper_sha256,
                chrono::Utc::now(),
            ))?;
        }
        Ok(())
    }

    fn record_enrollment_session(
        &self,
        request: &EnrollmentRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        record_bound_session_use(
            &request.actor,
            &request.public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            key,
        )
    }

    fn record_rebind_session(
        &self,
        request: &RebindRequest,
        key: &ed25519_dalek::SigningKey,
    ) -> Result<(), String> {
        record_bound_session_use(
            &request.actor,
            &request.public_key,
            &request.provider,
            request.protection_mode,
            &request.helper_sha256,
            key,
        )
    }
}

fn ensure_bound_session(
    actor: &str,
    public_key: &str,
    provider: &str,
    mode: ProtectionMode,
    helper_sha256: &str,
    operation: &str,
    prompt: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now();
    if load_session_record().is_some_and(|record| {
        record.state(
            actor,
            public_key,
            provider,
            protection_mode_name(mode),
            helper_sha256,
            now,
        ) == SessionState::Active
    }) {
        return Ok(());
    }
    require_user_interaction(operation)?;
    platform_reauthenticate(prompt)?;
    save_session_record(&SessionRecord::new(
        actor,
        public_key,
        provider,
        protection_mode_name(mode),
        helper_sha256,
        chrono::Utc::now(),
    ))
}

fn record_bound_session_use(
    actor: &str,
    public_key: &str,
    provider: &str,
    mode: ProtectionMode,
    helper_sha256: &str,
    key: &ed25519_dalek::SigningKey,
) -> Result<(), String> {
    let mut record =
        load_session_record().ok_or_else(|| "signer session disappeared".to_string())?;
    let now = chrono::Utc::now();
    record.touch(now)?;
    record.sign(key)?;
    if record.state(
        actor,
        public_key,
        provider,
        protection_mode_name(mode),
        helper_sha256,
        now,
    ) != SessionState::Active
    {
        return Err("signer session binding changed before completion".to_string());
    }
    save_session_record(&record)
}

fn require_user_interaction(operation: &str) -> Result<(), String> {
    if user_interaction_disabled(std::env::var_os("VELA_NO_USER_INTERACTION").as_deref()) {
        return Err(format!(
            "user interaction is disabled; refusing to {operation}"
        ));
    }
    Ok(())
}

fn user_interaction_disabled(value: Option<&std::ffi::OsStr>) -> bool {
    value
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|value| {
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HumanCard {
    title: String,
    description: String,
    action_label: String,
}

fn decision_card(request: &SignerRequest) -> HumanCard {
    let (title, action_label) = match request.action.as_str() {
        "accept" => ("Accept this result?", "Accept result"),
        "reject" => ("Reject this proposal?", "Reject proposal"),
        _ => ("Decide this proposal?", "Confirm decision"),
    };
    let facts = request
        .display
        .decisive_facts
        .iter()
        .map(|fact| format!("• {fact}"))
        .collect::<Vec<_>>()
        .join("\n");
    HumanCard {
        title: title.to_string(),
        description: format!(
            "{}\n\nWhy this needs your decision\n{}\n\nRationale\n{}\n\nWhat changes\n{}\n\nRequested by {} · {}\nProposal {} · Plan {}",
            request.display.claim,
            facts,
            request.reason,
            request.display.consequence,
            request.display.requester,
            request.display.frontier_name,
            short_id(&request.proposal_id),
            short_root(&request.decision_plan_root),
        ),
        action_label: action_label.to_string(),
    }
}

fn policy_decision_card(request: &PolicySignerRequest) -> HumanCard {
    let action = capitalize(request.action.as_str());
    let facts = request
        .display
        .decisive_facts
        .iter()
        .map(|fact| format!("• {fact}"))
        .collect::<Vec<_>>()
        .join("\n");
    HumanCard {
        title: format!("{action} this policy?"),
        description: format!(
            "{}\n\nExact scope\n{}\n\nRationale\n{}\n\nWhat changes\n{}\n\nFrontier {}\nPolicy {} · Plan {}",
            request.display.claim,
            facts,
            request.reason,
            request.display.consequence,
            request.display.frontier_name,
            short_id(&request.selected_policy_id),
            short_root(&request.decision_plan_root),
        ),
        action_label: format!("{action} policy"),
    }
}

fn short_id(value: &str) -> &str {
    &value[..value.len().min(20)]
}

fn short_root(value: &str) -> &str {
    &value[..value.len().min(15)]
}

fn card_approved(result: MessageDialogResult, action_label: &str) -> bool {
    matches!(result, MessageDialogResult::Custom(label) if label == action_label)
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
        Command::ApprovePolicy => approve_policy_once(),
        Command::Enroll => enroll_once(),
        Command::Rebind => rebind_once(),
    }
}

fn approve_policy_once() -> Result<(), String> {
    let mut bytes = read_request_bytes()?;
    let request: PolicySignerRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse closed policy signer request: {error}"))?;
    bytes.zeroize();
    let helper_path =
        std::env::current_exe().map_err(|error| format!("resolve running helper path: {error}"))?;
    let response = approve_and_sign_policy(
        &request,
        &SystemApproval,
        &SystemCustody::new(request.protection_mode),
        &helper_path,
        chrono::Utc::now(),
    )?;
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("serialize policy signer response: {error}"))?
    );
    Ok(())
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

fn rebind_once() -> Result<(), String> {
    let mut bytes = read_request_bytes()?;
    let request: RebindRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse closed rebind request: {error}"))?;
    bytes.zeroize();
    let helper_path =
        std::env::current_exe().map_err(|error| format!("resolve running helper path: {error}"))?;
    let response = rebind(
        &request,
        &SystemApproval,
        &SystemCustody::new(request.protection_mode),
        &helper_path,
        chrono::Utc::now(),
    )?;
    println!(
        "{}",
        serde_json::to_string(&response)
            .map_err(|error| format!("serialize rebind response: {error}"))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SignerDisplay, SignerEvent};

    fn request(action: &str) -> SignerRequest {
        let event = vela_protocol::events::new_review_decision_event(
            "vpr_0123456789abcdef",
            "finding.add",
            if action == "accept" {
                "accepted"
            } else {
                "rejected"
            },
            None,
            "reviewer:fixture",
            "The result lacks independent evidence.",
            Some("2026-07-17T12:00:00Z"),
        )
        .unwrap();
        SignerRequest {
            schema: crate::contract::REQUEST_SCHEMA.to_string(),
            nonce: "1".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: "/bin/vela".to_string(),
            vela_binary_sha256: format!("sha256:{}", "2".repeat(64)),
            helper_sha256: format!("sha256:{}", "3".repeat(64)),
            frontier_id: "vfr_fixture".to_string(),
            frontier_path: "/tmp/frontier".to_string(),
            proposal_id: "vpr_0123456789abcdef".to_string(),
            proposal_root: format!("sha256:{}", "4".repeat(64)),
            action: action.to_string(),
            reason: "The result lacks independent evidence.".to_string(),
            reviewer_actor: "reviewer:fixture".to_string(),
            reviewer_public_key: "5".repeat(64),
            observed_at: "2026-07-17T12:00:00Z".to_string(),
            decision_plan_root: format!("sha256:{}", "6".repeat(64)),
            gate_state: "accept_ready=false;reject_ready=true".to_string(),
            provider: "os_store".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: ProtectionMode::Session,
            display: SignerDisplay {
                frontier_name: "Erdos problems".to_string(),
                claim: "No witness occurs in the bounded interval".to_string(),
                requester: "agent:canopus".to_string(),
                decisive_facts: vec![
                    "No independent verifier attachments".to_string(),
                    "No surviving adversarial probe".to_string(),
                ],
                consequence:
                    "Keep accepted scientific state unchanged and close this proposal as rejected."
                        .to_string(),
            },
            events: vec![SignerEvent { event }],
        }
    }

    #[test]
    fn decision_card_uses_semantic_actions_and_hides_custody_internals() {
        let request = request("reject");
        let card = decision_card(&request);
        assert_eq!(card.title, "Reject this proposal?");
        assert_eq!(card.action_label, "Reject proposal");
        assert!(card.description.contains(&request.display.claim));
        assert!(
            card.description
                .contains("No independent verifier attachments")
        );
        assert!(
            card.description
                .contains("Keep accepted scientific state unchanged")
        );
        assert!(card.description.contains("agent:canopus"));
        assert!(!card.description.contains(&request.proposal_root));
        assert!(!card.description.contains(&request.reviewer_public_key));
        assert!(!card.description.contains("os_store"));
        assert!(!card.description.contains("Yes"));
    }

    #[test]
    fn accept_and_reject_cards_cannot_share_an_action_label() {
        let accept = decision_card(&request("accept"));
        let reject = decision_card(&request("reject"));
        assert_eq!(accept.action_label, "Accept result");
        assert_eq!(reject.action_label, "Reject proposal");
        assert_ne!(accept.title, reject.title);
    }

    #[test]
    fn only_the_exact_custom_action_can_approve() {
        assert!(card_approved(
            MessageDialogResult::Custom("Reject proposal".to_string()),
            "Reject proposal"
        ));
        assert!(!card_approved(MessageDialogResult::Ok, "Reject proposal"));
        assert!(!card_approved(
            MessageDialogResult::Custom("Cancel".to_string()),
            "Reject proposal"
        ));
        assert!(!card_approved(
            MessageDialogResult::Custom("Accept result".to_string()),
            "Reject proposal"
        ));
    }

    #[test]
    fn automated_context_can_disable_every_system_prompt() {
        for value in ["1", "true", "TRUE", "yes", "YES"] {
            assert!(user_interaction_disabled(Some(std::ffi::OsStr::new(value))));
        }
        for value in ["", "0", "false", "no"] {
            assert!(!user_interaction_disabled(Some(std::ffi::OsStr::new(
                value
            ))));
        }
        assert!(!user_interaction_disabled(None));
    }
}
