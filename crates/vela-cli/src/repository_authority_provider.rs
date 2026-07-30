//! Standard repository-authority signer providers.
//!
//! The first provider uses only the standard OpenSSH agent
//! request-identities and sign-request messages. It never reads a private-key
//! file and receives only the canonical authority-record payload after the
//! writer has completed every authentication, authorization, and semantic
//! check. Fresh authority initialization and ordinary Era-1 transactions use
//! the same provider boundary.

use std::env;
use std::io::{Read, Write};
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use vela_protocol::authority::{AUTHORITY_PAYLOAD_TYPE_V1, DsseSignatureV1, dsse_pae};

use crate::authority_transaction::RepositoryAuthoritySigner;

const SSH_AGENT_FAILURE: u8 = 5;
const SSH2_AGENTC_REQUEST_IDENTITIES: u8 = 11;
const SSH2_AGENT_IDENTITIES_ANSWER: u8 = 12;
const SSH2_AGENTC_SIGN_REQUEST: u8 = 13;
const SSH2_AGENT_SIGN_RESPONSE: u8 = 14;
const SSH_ED25519: &[u8] = b"ssh-ed25519";
const MAX_AGENT_MESSAGE: usize = 1024 * 1024;

trait AgentIo: Read + Write {}
impl<T: Read + Write> AgentIo for T {}

struct AgentConnection {
    stream: Box<dyn AgentIo>,
}

impl AgentConnection {
    fn connect(path: &Path) -> Result<Self, String> {
        #[cfg(unix)]
        {
            let stream = std::os::unix::net::UnixStream::connect(path)
                .map_err(|error| format!("connect to SSH agent: {error}"))?;
            return Ok(Self {
                stream: Box::new(stream),
            });
        }
        #[cfg(windows)]
        {
            let stream = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|error| format!("connect to SSH agent named pipe: {error}"))?;
            return Ok(Self {
                stream: Box::new(stream),
            });
        }
        #[allow(unreachable_code)]
        Err("SSH agent transport is unsupported on this platform".into())
    }

    fn exchange(&mut self, message: &[u8]) -> Result<Vec<u8>, String> {
        if message.len() > MAX_AGENT_MESSAGE {
            return Err("SSH agent request exceeds the bounded message size".into());
        }
        self.stream
            .write_all(&(message.len() as u32).to_be_bytes())
            .and_then(|()| self.stream.write_all(message))
            .and_then(|()| self.stream.flush())
            .map_err(|error| format!("write SSH agent request: {error}"))?;
        let mut length = [0_u8; 4];
        self.stream
            .read_exact(&mut length)
            .map_err(|error| format!("read SSH agent response length: {error}"))?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 || length > MAX_AGENT_MESSAGE {
            return Err(format!("SSH agent response length {length} is invalid"));
        }
        let mut response = vec![0_u8; length];
        self.stream
            .read_exact(&mut response)
            .map_err(|error| format!("read SSH agent response: {error}"))?;
        if response[0] == SSH_AGENT_FAILURE {
            return Err("SSH agent refused the request".into());
        }
        Ok(response)
    }

    fn identities(&mut self) -> Result<Vec<AgentIdentity>, String> {
        let response = self.exchange(&[SSH2_AGENTC_REQUEST_IDENTITIES])?;
        let mut cursor = SshCursor::new(&response);
        cursor.expect_byte(SSH2_AGENT_IDENTITIES_ANSWER, "identities response")?;
        let count = cursor.read_u32("identity count")? as usize;
        if count > 4096 {
            return Err("SSH agent identity count exceeds the bounded maximum".into());
        }
        let mut identities = Vec::with_capacity(count);
        for _ in 0..count {
            let key_blob = cursor.read_string("identity key")?.to_vec();
            let _comment = cursor.read_string("identity comment")?;
            identities.push(AgentIdentity::parse(key_blob)?);
        }
        cursor.finish("identities response")?;
        Ok(identities)
    }

    fn sign(&mut self, key_blob: &[u8], message: &[u8]) -> Result<[u8; 64], String> {
        let mut request = vec![SSH2_AGENTC_SIGN_REQUEST];
        push_string(&mut request, key_blob)?;
        push_string(&mut request, message)?;
        request.extend_from_slice(&0_u32.to_be_bytes());
        let response = self.exchange(&request)?;
        let mut cursor = SshCursor::new(&response);
        cursor.expect_byte(SSH2_AGENT_SIGN_RESPONSE, "signature response")?;
        let signature_blob = cursor.read_string("signature blob")?;
        cursor.finish("signature response")?;

        let mut signature = SshCursor::new(signature_blob);
        if signature.read_string("signature algorithm")? != SSH_ED25519 {
            return Err("SSH agent returned a non-Ed25519 signature".into());
        }
        let bytes = signature.read_string("Ed25519 signature")?;
        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| "SSH agent returned a non-64-byte Ed25519 signature".to_string())?;
        signature.finish("Ed25519 signature blob")?;
        Ok(bytes)
    }
}

#[derive(Clone)]
struct AgentIdentity {
    key_blob: Vec<u8>,
    ed25519_public_key: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryAuthorityIdentity {
    pub(crate) key_id: String,
    pub(crate) fingerprint: String,
    pub(crate) public_key: String,
}

impl AgentIdentity {
    fn parse(key_blob: Vec<u8>) -> Result<Self, String> {
        let mut cursor = SshCursor::new(&key_blob);
        let algorithm = cursor.read_string("identity algorithm")?;
        let ed25519_public_key = if algorithm == SSH_ED25519 {
            let key = cursor.read_string("Ed25519 public key")?;
            Some(
                key.try_into()
                    .map_err(|_| "SSH agent returned a malformed Ed25519 key".to_string())?,
            )
        } else {
            None
        };
        if ed25519_public_key.is_some() {
            cursor.finish("Ed25519 identity")?;
        }
        Ok(Self {
            key_blob,
            ed25519_public_key,
        })
    }
}

struct SshCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> SshCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn expect_byte(&mut self, expected: u8, label: &str) -> Result<(), String> {
        let actual = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| format!("SSH agent {label} is truncated"))?;
        self.position += 1;
        if actual != expected {
            return Err(format!(
                "SSH agent {label} has message type {actual}, expected {expected}"
            ));
        }
        Ok(())
    }

    fn read_u32(&mut self, label: &str) -> Result<u32, String> {
        let end = self
            .position
            .checked_add(4)
            .ok_or_else(|| format!("SSH agent {label} length overflow"))?;
        let bytes: [u8; 4] = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("SSH agent {label} is truncated"))?
            .try_into()
            .expect("four-byte slice");
        self.position = end;
        Ok(u32::from_be_bytes(bytes))
    }

    fn read_string(&mut self, label: &str) -> Result<&'a [u8], String> {
        let length = self.read_u32(label)? as usize;
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| format!("SSH agent {label} length overflow"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("SSH agent {label} is truncated"))?;
        self.position = end;
        Ok(value)
    }

    fn finish(&self, label: &str) -> Result<(), String> {
        if self.position != self.bytes.len() {
            return Err(format!("SSH agent {label} has trailing bytes"));
        }
        Ok(())
    }
}

fn push_string(output: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let length =
        u32::try_from(value.len()).map_err(|_| "SSH agent field exceeds u32".to_string())?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

/// OpenSSH-agent-backed Ed25519 repository authority.
///
/// The configured public key must appear exactly once as a plain Ed25519
/// identity in the selected agent. Certificates, security-key signatures, RSA
/// keys, and algorithm substitution are rejected because
/// `vela.authority-keyset.v1` currently admits only raw Ed25519 verification.
pub(crate) struct SshAgentRepositoryAuthoritySigner {
    connection: Option<AgentConnection>,
    identity: Option<AgentIdentity>,
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
        let mut connection = AgentConnection::connect(socket_path)?;
        let matches = connection
            .identities()?
            .into_iter()
            .filter(|identity| identity.ed25519_public_key == Some(expected_public_key))
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
        let socket = env::var_os("SSH_AUTH_SOCK")
            .or({
                #[cfg(windows)]
                {
                    Some(r"\\.\pipe\openssh-ssh-agent".into())
                }
                #[cfg(not(windows))]
                {
                    None
                }
            })
            .ok_or_else(|| "SSH_AUTH_SOCK is not set".to_string())?;
        let connected = Self::connect(
            Path::new(&socket),
            self.key_id.clone(),
            &hex::encode(self.expected_public_key),
        )?;
        self.connection = connected.connection;
        self.identity = connected.identity;
        Ok(())
    }
}

pub(crate) fn select_repository_authority_identity(
    selector: Option<&str>,
) -> Result<RepositoryAuthorityIdentity, String> {
    let socket = env::var_os("SSH_AUTH_SOCK")
        .or({
            #[cfg(windows)]
            {
                Some(r"\\.\pipe\openssh-ssh-agent".into())
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
        .ok_or_else(|| {
            "no standard OpenSSH agent is available; start one and load one dedicated Ed25519 repository-authority key"
                .to_string()
        })?;
    let mut connection = AgentConnection::connect(Path::new(&socket))?;
    let mut identities = connection
        .identities()?
        .into_iter()
        .filter_map(|identity| {
            identity.ed25519_public_key.map(|public_key| {
                let fingerprint = format!(
                    "SHA256:{}",
                    STANDARD_NO_PAD.encode(Sha256::digest(&identity.key_blob))
                );
                RepositoryAuthorityIdentity {
                    key_id: format!("ssh-ed25519:{fingerprint}"),
                    fingerprint,
                    public_key: hex::encode(public_key),
                }
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
        let signature = Signature::from_bytes(
            &connection
                .sign(&identity.key_blob, &pae)
                .map_err(|error| format!("SSH agent signing failed: {error}"))?,
        );
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
        let key_blob = BASE64_STANDARD.decode(encoded).unwrap();
        AgentIdentity::parse(key_blob)
            .unwrap()
            .ed25519_public_key
            .unwrap()
    }

    #[test]
    fn ssh_agent_provider_signs_only_the_exact_dsse_pae_with_the_bound_key() {
        let guard = start_agent();
        let public_key = load_fixture_identity(&guard);
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
