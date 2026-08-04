//! Standard repository-authority signer providers.
//!
//! The first provider uses only the standard OpenSSH agent
//! request-identities and sign-request messages. It never reads a private-key
//! file and receives only the canonical authority-record payload after the
//! writer has completed every authentication, authorization, and semantic
//! check. Fresh authority initialization and ordinary Era-1 transactions use
//! the same provider boundary.

use std::env;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ssh_encoding::{Decode, Encode, Reader};
use ssh_key::Signature as SshSignature;
use ssh_key::public::KeyData;
use ssh_key::{Algorithm as SshAlgorithm, HashAlg};
use vela_protocol::authority::{AUTHORITY_PAYLOAD_TYPE_V1, DsseSignatureV1, dsse_pae};

use crate::authority_transaction::RepositoryAuthoritySigner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryAuthorityIdentity {
    pub(crate) key_id: String,
    pub(crate) fingerprint: String,
    pub(crate) public_key: String,
}

const SSH_AGENT_FAILURE: u8 = 5;
const SSH2_AGENTC_REQUEST_IDENTITIES: u8 = 11;
const SSH2_AGENT_IDENTITIES_ANSWER: u8 = 12;
const SSH2_AGENTC_SIGN_REQUEST: u8 = 13;
const SSH2_AGENT_SIGN_RESPONSE: u8 = 14;
const MAX_AGENT_FRAME_LEN: usize = 1024 * 1024 - 1;
const MAX_AGENT_IDENTITIES: u32 = 256;

#[derive(Clone, Debug)]
struct OpenSshAgentIdentity {
    /// Exact public-key blob returned by the agent and sent back for signing.
    blob: Vec<u8>,
    key_data: KeyData,
}

/// The deliberately small part of the OpenSSH agent protocol Vela needs.
///
/// Cryptographic parsing and encoding remain in RustCrypto's `ssh-key` and
/// `ssh-encoding` crates. This adapter owns only Unix-socket framing for the
/// standard request-identities and sign-request messages.
#[cfg(unix)]
struct OpenSshAgentClient {
    stream: UnixStream,
}

#[cfg(unix)]
impl OpenSshAgentClient {
    fn connect(socket_path: &Path) -> Result<Self, String> {
        let stream = UnixStream::connect(socket_path)
            .map_err(|error| format!("connect to SSH agent: {error}"))?;
        let timeout = Some(Duration::from_secs(10));
        stream
            .set_read_timeout(timeout)
            .map_err(|error| format!("set SSH agent read timeout: {error}"))?;
        stream
            .set_write_timeout(timeout)
            .map_err(|error| format!("set SSH agent write timeout: {error}"))?;
        Ok(Self { stream })
    }

    fn request_identities(&mut self) -> Result<Vec<OpenSshAgentIdentity>, String> {
        let response = self.exchange(&[SSH2_AGENTC_REQUEST_IDENTITIES])?;
        let mut reader = response.as_slice();
        let message = u8::decode(&mut reader)
            .map_err(|error| format!("decode SSH agent identity response: {error}"))?;
        if message == SSH_AGENT_FAILURE {
            return Err("SSH agent refused the identity request".into());
        }
        if message != SSH2_AGENT_IDENTITIES_ANSWER {
            return Err(format!(
                "SSH agent returned unexpected identity response {message}"
            ));
        }
        let count = u32::decode(&mut reader)
            .map_err(|error| format!("decode SSH agent identity count: {error}"))?;
        if count > MAX_AGENT_IDENTITIES {
            return Err(format!(
                "SSH agent returned {count} identities, maximum is {MAX_AGENT_IDENTITIES}"
            ));
        }
        let mut identities = Vec::new();
        for _ in 0..count {
            let blob = Vec::<u8>::decode(&mut reader)
                .map_err(|error| format!("decode SSH agent identity blob: {error}"))?;
            let _comment = Vec::<u8>::decode(&mut reader)
                .map_err(|error| format!("decode SSH agent identity comment: {error}"))?;
            let mut key_reader = blob.as_slice();
            if let Ok(key_data) = KeyData::decode(&mut key_reader)
                && key_reader.finish(()).is_ok()
                && key_data.ed25519().is_some()
            {
                identities.push(OpenSshAgentIdentity { blob, key_data });
            }
        }
        reader
            .finish(identities)
            .map_err(|error| format!("decode SSH agent identity response: {error}"))
    }

    fn sign(
        &mut self,
        identity: &OpenSshAgentIdentity,
        data: &[u8],
    ) -> Result<SshSignature, String> {
        let mut request = vec![SSH2_AGENTC_SIGN_REQUEST];
        identity
            .blob
            .as_slice()
            .encode(&mut request)
            .map_err(|error| format!("encode SSH agent identity: {error}"))?;
        data.encode(&mut request)
            .map_err(|error| format!("encode SSH agent signing payload: {error}"))?;
        0u32.encode(&mut request)
            .map_err(|error| format!("encode SSH agent signing flags: {error}"))?;

        let response = self.exchange(&request)?;
        let mut reader = response.as_slice();
        let message = u8::decode(&mut reader)
            .map_err(|error| format!("decode SSH agent signing response: {error}"))?;
        if message == SSH_AGENT_FAILURE {
            return Err("SSH agent refused the signing request".into());
        }
        if message != SSH2_AGENT_SIGN_RESPONSE {
            return Err(format!(
                "SSH agent returned unexpected signing response {message}"
            ));
        }
        let signature_blob = Vec::<u8>::decode(&mut reader)
            .map_err(|error| format!("decode SSH agent signature blob: {error}"))?;
        reader
            .finish(())
            .map_err(|error| format!("decode SSH agent signing response: {error}"))?;
        let mut signature_reader = signature_blob.as_slice();
        let signature = SshSignature::decode(&mut signature_reader)
            .map_err(|error| format!("decode SSH agent signature: {error}"))?;
        signature_reader
            .finish(signature)
            .map_err(|error| format!("decode SSH agent signature: {error}"))
    }

    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, String> {
        if request.is_empty() || request.len() > MAX_AGENT_FRAME_LEN {
            return Err(format!(
                "SSH agent request is {} bytes, expected 1..={MAX_AGENT_FRAME_LEN}",
                request.len()
            ));
        }
        let request_len = u32::try_from(request.len())
            .map_err(|_| "SSH agent request length exceeds uint32".to_string())?;
        self.stream
            .write_all(&request_len.to_be_bytes())
            .and_then(|()| self.stream.write_all(request))
            .map_err(|error| format!("write SSH agent request: {error}"))?;

        let mut header = [0u8; 4];
        self.stream
            .read_exact(&mut header)
            .map_err(|error| format!("read SSH agent response length: {error}"))?;
        let response_len = usize::try_from(u32::from_be_bytes(header))
            .map_err(|_| "SSH agent response length exceeds usize".to_string())?;
        if response_len == 0 || response_len > MAX_AGENT_FRAME_LEN {
            return Err(format!(
                "SSH agent response is {response_len} bytes, expected 1..={MAX_AGENT_FRAME_LEN}"
            ));
        }
        let mut response = vec![0u8; response_len];
        self.stream
            .read_exact(&mut response)
            .map_err(|error| format!("read SSH agent response: {error}"))?;
        Ok(response)
    }
}

/// OpenSSH-agent-backed Ed25519 repository authority.
///
/// The configured public key must appear exactly once as a plain Ed25519
/// identity in the selected agent. Certificates, security-key signatures, RSA
/// keys, and algorithm substitution are rejected because
/// `vela.authority-keyset.v1` currently admits only raw Ed25519 verification.
pub(crate) struct SshAgentRepositoryAuthoritySigner {
    connection: Option<OpenSshAgentClient>,
    identity: Option<OpenSshAgentIdentity>,
    key_id: String,
    expected_public_key: [u8; 32],
}

impl SshAgentRepositoryAuthoritySigner {
    pub(crate) fn connect(
        socket_path: &Path,
        key_id: impl Into<String>,
        expected_public_key_hex: &str,
    ) -> Result<Self, String> {
        let key_id = key_id.into();
        if key_id.trim().is_empty() {
            return Err("repository-authority key ID is empty".into());
        }
        if socket_path.as_os_str().is_empty() {
            return Err("SSH agent socket path is empty".into());
        }
        let expected_public_key = decode_ed25519_public_key(expected_public_key_hex)?;
        let mut connection = OpenSshAgentClient::connect(socket_path)?;
        let matches = connection
            .request_identities()
            .map_err(|error| format!("list SSH agent identities: {error}"))?
            .into_iter()
            .filter(|identity| ed25519_public_key(identity) == Some(expected_public_key))
            .collect::<Vec<_>>();
        let [identity] = matches.as_slice() else {
            return Err(format!(
                "SSH agent must expose exactly one plain Ed25519 identity matching key {key_id}; found {}",
                matches.len()
            ));
        };
        Ok(Self {
            connection: Some(connection),
            identity: Some(identity.clone()),
            key_id,
            expected_public_key,
        })
    }

    /// Prepare the platform's standard OpenSSH agent signer.
    ///
    /// This constructor validates only the static authority-key configuration.
    /// It deliberately defers endpoint resolution, identity enumeration, and
    /// agent access until `sign`, after transaction authentication and Cedar
    /// authorization have passed. The endpoint itself is process-local
    /// configuration and never enters a Frontier, an authority record, a
    /// journal, or a diagnostic payload.
    pub(crate) fn from_environment(
        key_id: impl Into<String>,
        expected_public_key_hex: &str,
    ) -> Result<Self, String> {
        let key_id = key_id.into();
        if key_id.trim().is_empty() {
            return Err("repository-authority key ID is empty".into());
        }
        let expected_public_key = decode_ed25519_public_key(expected_public_key_hex)?;
        Ok(Self {
            connection: None,
            identity: None,
            key_id,
            expected_public_key,
        })
    }

    fn connect_from_environment(&mut self) -> Result<(), String> {
        if self.connection.is_some() && self.identity.is_some() {
            return Ok(());
        }
        if self.connection.is_some() || self.identity.is_some() {
            return Err("repository-authority signer has partial agent state".into());
        }
        let sockets = ssh_agent_sockets();
        if sockets.is_empty() {
            return Err(
                "repository authority signer is unavailable because no standard OpenSSH agent socket is available; load the dedicated repository key once into the login session"
                    .into(),
            );
        }
        let mut failures = Vec::new();
        for socket in sockets {
            match Self::connect(
                &socket,
                self.key_id.clone(),
                &hex::encode(self.expected_public_key),
            ) {
                Ok(connected) => {
                    self.connection = connected.connection;
                    self.identity = connected.identity;
                    return Ok(());
                }
                Err(error) => failures.push(error),
            }
        }
        Err(format!(
            "repository authority signer could not use the current OpenSSH agent session: {}",
            failures.join("; ")
        ))
    }
}

/// Resolve the standard agent endpoints available to this login session.
///
/// GUI applications can outlive the shell that refreshed `SSH_AUTH_SOCK`.
/// macOS launchd owns the login-session endpoint, so consult it when the
/// inherited process environment is missing or stale. No private key or
/// repository configuration is read during endpoint discovery.
fn ssh_agent_sockets() -> Vec<PathBuf> {
    if let Some(socket) = env::var_os("SSH_AUTH_SOCK").filter(|value| !value.is_empty()) {
        return vec![PathBuf::from(socket)];
    }
    #[cfg(target_os = "macos")]
    let mut sockets = Vec::new();
    #[cfg(not(target_os = "macos"))]
    let sockets = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(output) = Command::new("/bin/launchctl")
        .args(["getenv", "SSH_AUTH_SOCK"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        && output.status.success()
        && let Ok(value) = String::from_utf8(output.stdout)
    {
        let value = value.trim();
        if !value.is_empty() {
            let socket = PathBuf::from(value);
            if !sockets.contains(&socket) {
                sockets.push(socket);
            }
        }
    }
    sockets
}

pub(crate) fn select_repository_authority_identity(
    selector: Option<&str>,
) -> Result<RepositoryAuthorityIdentity, String> {
    let sockets = ssh_agent_sockets();
    let mut connection = sockets
        .iter()
        .find_map(|socket| OpenSshAgentClient::connect(socket).ok())
        .ok_or_else(|| {
            "no standard OpenSSH agent is available; start one and load one dedicated Ed25519 repository-authority key"
                .to_string()
        })?;
    let mut identities = connection
        .request_identities()
        .map_err(|error| format!("list SSH agent identities: {error}"))?
        .into_iter()
        .filter_map(|identity| {
            let ed25519 = identity.key_data.ed25519()?;
            let public_key_bytes = ed25519.0;
            let fingerprint = identity.key_data.fingerprint(HashAlg::Sha256).to_string();
            Some(RepositoryAuthorityIdentity {
                key_id: format!("ssh-ed25519:{fingerprint}"),
                fingerprint,
                public_key: hex::encode(public_key_bytes),
            })
        })
        .collect::<Vec<_>>();
    identities.sort_by(|left, right| left.key_id.cmp(&right.key_id));
    let matches = identities
        .iter()
        .filter(|identity| {
            selector.is_none_or(|selector| {
                selector == identity.key_id
                    || selector == identity.fingerprint
                    || selector == identity.public_key
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [identity] => Ok(identity.clone()),
        [] if identities.is_empty() => Err(
            "the OpenSSH agent exposes no plain Ed25519 identity; load one dedicated repository-authority key"
                .into(),
        ),
        [] => Err(format!(
            "no loaded Ed25519 identity matches {}; available fingerprints: {}",
            selector.unwrap_or("<automatic selection>"),
            identities
                .iter()
                .map(|identity| identity.fingerprint.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        _ => Err(format!(
            "the OpenSSH agent exposes multiple Ed25519 identities; select one with --key <fingerprint>: {}",
            matches
                .iter()
                .map(|identity| identity.fingerprint.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

impl RepositoryAuthoritySigner for SshAgentRepositoryAuthoritySigner {
    fn sign(
        &mut self,
        payload_type: &str,
        canonical_payload: &[u8],
    ) -> Result<Vec<DsseSignatureV1>, String> {
        if payload_type != AUTHORITY_PAYLOAD_TYPE_V1 {
            return Err(format!(
                "repository authority refuses payload type {payload_type}"
            ));
        }
        self.connect_from_environment()?;
        let pae = dsse_pae(payload_type, canonical_payload);
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| "repository-authority signer has no agent connection".to_string())?;
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| "repository-authority signer has no selected identity".to_string())?;
        let agent_signature = connection
            .sign(identity, &pae)
            .map_err(|error| format!("SSH agent signing failed: {error}"))?;
        if agent_signature.algorithm() != SshAlgorithm::Ed25519 {
            return Err("SSH agent returned a non-Ed25519 signature".into());
        }
        let signature_bytes: [u8; 64] = agent_signature
            .as_bytes()
            .try_into()
            .map_err(|_| "SSH agent returned a non-64-byte Ed25519 signature".to_string())?;
        let signature = Signature::from_bytes(&signature_bytes);
        let verifying_key = VerifyingKey::from_bytes(&self.expected_public_key)
            .map_err(|error| format!("configured repository public key is invalid: {error}"))?;
        verifying_key.verify(&pae, &signature).map_err(|error| {
            format!("SSH agent signature does not match the configured key: {error}")
        })?;
        Ok(vec![DsseSignatureV1 {
            keyid: self.key_id.clone(),
            sig: BASE64_STANDARD.encode(signature.to_bytes()),
        }])
    }
}

fn ed25519_public_key(identity: &OpenSshAgentIdentity) -> Option<[u8; 32]> {
    identity.key_data.ed25519().map(|key| key.0)
}

fn decode_ed25519_public_key(value: &str) -> Result<[u8; 32], String> {
    let decoded =
        hex::decode(value).map_err(|error| format!("decode repository public key: {error}"))?;
    decoded.try_into().map_err(|bytes: Vec<u8>| {
        format!(
            "repository public key is {} bytes, expected 32",
            bytes.len()
        )
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    struct AgentGuard {
        child: Child,
        socket: std::path::PathBuf,
        directory: TempDir,
    }

    impl Drop for AgentGuard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = fs::remove_file(&self.socket);
        }
    }

    fn start_agent() -> AgentGuard {
        let directory = tempfile::Builder::new()
            .prefix("vela-agent-")
            .tempdir_in("/tmp")
            .unwrap();
        let socket = directory.path().join("agent.sock");
        let child = Command::new("ssh-agent")
            .arg("-D")
            .arg("-a")
            .arg(&socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("OpenSSH ssh-agent is required for the provider qualification");
        let mut guard = AgentGuard {
            child,
            socket,
            directory,
        };
        for _ in 0..100 {
            if guard.socket.exists() {
                return guard;
            }
            if let Some(status) = guard.child.try_wait().unwrap() {
                panic!("ssh-agent exited before creating its socket: {status}");
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("ssh-agent did not create its socket");
    }

    fn load_fixture_identity(guard: &AgentGuard) -> [u8; 32] {
        let private_key = guard.directory.path().join("fixture");
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&private_key)
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("ssh-add")
            .arg(&private_key)
            .env("SSH_AUTH_SOCK", &guard.socket)
            .env("SSH_ASKPASS_REQUIRE", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());

        let public = fs::read_to_string(private_key.with_extension("pub")).unwrap();
        let encoded = public.split_whitespace().nth(1).unwrap();
        let key = ssh_key::PublicKey::from_openssh(&format!("ssh-ed25519 {encoded}")).unwrap();
        key.key_data().ed25519().unwrap().0
    }

    fn load_unsupported_fixture_identity(guard: &AgentGuard) {
        let private_key = guard.directory.path().join("unsupported-rsa-fixture");
        let status = Command::new("ssh-keygen")
            .args(["-q", "-t", "rsa", "-b", "2048", "-N", "", "-f"])
            .arg(&private_key)
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("ssh-add")
            .arg(&private_key)
            .env("SSH_AUTH_SOCK", &guard.socket)
            .env("SSH_ASKPASS_REQUIRE", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn client_with_response(response: Vec<u8>) -> (OpenSshAgentClient, thread::JoinHandle<()>) {
        let (client, mut server) = UnixStream::pair().unwrap();
        let handle = thread::spawn(move || {
            let mut request_len = [0u8; 4];
            server.read_exact(&mut request_len).unwrap();
            let mut request = vec![0u8; u32::from_be_bytes(request_len) as usize];
            server.read_exact(&mut request).unwrap();
            server
                .write_all(&(response.len() as u32).to_be_bytes())
                .unwrap();
            server.write_all(&response).unwrap();
        });
        (OpenSshAgentClient { stream: client }, handle)
    }

    #[test]
    fn ssh_agent_adapter_ignores_unsupported_identity_algorithms() {
        let mut unsupported_blob = Vec::new();
        b"future-key@example.test"
            .as_slice()
            .encode(&mut unsupported_blob)
            .unwrap();
        b"opaque-key-data"
            .as_slice()
            .encode(&mut unsupported_blob)
            .unwrap();
        let mut response = vec![SSH2_AGENT_IDENTITIES_ANSWER];
        1u32.encode(&mut response).unwrap();
        unsupported_blob.as_slice().encode(&mut response).unwrap();
        b"unsupported fixture"
            .as_slice()
            .encode(&mut response)
            .unwrap();
        let (mut client, handle) = client_with_response(response);

        assert!(client.request_identities().unwrap().is_empty());
        handle.join().unwrap();
    }

    #[test]
    fn ssh_agent_adapter_rejects_oversized_identity_sets_and_trailing_data() {
        let mut oversized = vec![SSH2_AGENT_IDENTITIES_ANSWER];
        (MAX_AGENT_IDENTITIES + 1).encode(&mut oversized).unwrap();
        let (mut client, handle) = client_with_response(oversized);
        let error = client.request_identities().unwrap_err();
        assert!(error.contains("maximum"), "{error}");
        handle.join().unwrap();

        let mut trailing = vec![SSH2_AGENT_IDENTITIES_ANSWER];
        0u32.encode(&mut trailing).unwrap();
        trailing.push(0xff);
        let (mut client, handle) = client_with_response(trailing);
        let error = client.request_identities().unwrap_err();
        assert!(error.contains("trailing data"), "{error}");
        handle.join().unwrap();
    }

    #[test]
    fn ssh_agent_provider_signs_only_the_exact_dsse_pae_with_the_bound_key() {
        let guard = start_agent();
        let public_key = load_fixture_identity(&guard);
        load_unsupported_fixture_identity(&guard);
        let mut provider = SshAgentRepositoryAuthoritySigner::connect(
            &guard.socket,
            "repository-key-1",
            &hex::encode(public_key),
        )
        .unwrap();
        let payload = br#"{"schema":"vela.authority-record.v1"}"#;
        let signatures = provider.sign(AUTHORITY_PAYLOAD_TYPE_V1, payload).unwrap();
        assert_eq!(signatures.len(), 1);
        assert_eq!(signatures[0].keyid, "repository-key-1");
        let signature_bytes = BASE64_STANDARD.decode(&signatures[0].sig).unwrap();
        let signature = Signature::try_from(signature_bytes.as_slice()).unwrap();
        VerifyingKey::from_bytes(&public_key)
            .unwrap()
            .verify(&dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, payload), &signature)
            .unwrap();

        let second_payload = br#"{"schema":"vela.authority-record.v1","sequence":2}"#;
        let second = provider
            .sign(AUTHORITY_PAYLOAD_TYPE_V1, second_payload)
            .unwrap();
        assert_eq!(second.len(), 1);
        let second_signature =
            Signature::try_from(BASE64_STANDARD.decode(&second[0].sig).unwrap().as_slice())
                .unwrap();
        VerifyingKey::from_bytes(&public_key)
            .unwrap()
            .verify(
                &dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, second_payload),
                &second_signature,
            )
            .unwrap();

        let error = provider.sign("application/json", payload).unwrap_err();
        assert!(error.contains("payload type"), "{error}");
    }

    #[test]
    fn ssh_agent_provider_rejects_missing_or_substituted_identity() {
        let guard = start_agent();
        let public_key = load_fixture_identity(&guard);
        let error = SshAgentRepositoryAuthoritySigner::connect(
            &guard.socket,
            "repository-key-1",
            &hex::encode([43; 32]),
        )
        .err()
        .expect("wrong identity must fail");
        assert!(error.contains("found 0"), "{error}");

        let provider = SshAgentRepositoryAuthoritySigner::connect(
            &guard.socket,
            "repository-key-1",
            &hex::encode(public_key),
        );
        assert!(provider.is_ok());
    }

    #[test]
    fn ssh_agent_provider_refuses_malformed_configuration_before_agent_use() {
        let error = SshAgentRepositoryAuthoritySigner::connect(
            Path::new("/definitely/missing/ssh-agent.sock"),
            "",
            &"00".repeat(32),
        )
        .err()
        .expect("empty key id must fail");
        assert!(error.contains("key ID"), "{error}");

        let error = SshAgentRepositoryAuthoritySigner::connect(
            Path::new("/definitely/missing/ssh-agent.sock"),
            "repository-key-1",
            "00",
        )
        .err()
        .expect("wrong key length must fail");
        assert!(error.contains("expected 32"), "{error}");
    }

    #[test]
    fn environment_provider_defers_agent_access_until_signing() {
        let mut provider = SshAgentRepositoryAuthoritySigner::from_environment(
            "repository-key-1",
            &hex::encode([42; 32]),
        )
        .unwrap();
        assert!(provider.connection.is_none());
        assert!(provider.identity.is_none());

        let error = provider
            .sign("application/json", br#"{"schema":"not-authority"}"#)
            .unwrap_err();
        assert!(error.contains("payload type"), "{error}");
        assert!(provider.connection.is_none());
        assert!(provider.identity.is_none());
    }
}
