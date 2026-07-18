use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use std::io::Read;
use zeroize::Zeroize;

use crate::contract::{
    ENROLLMENT_RESPONSE_SCHEMA, EnrollmentRequest, EnrollmentResponse, EventSignature,
    ProtectionMode, REBIND_RESPONSE_SCHEMA, RESPONSE_SCHEMA, RebindRequest, RebindResponse,
    SignerRequest, SignerResponse, file_sha256, rebind_request_root, rebind_response_signing_bytes,
    request_root, validate_enrollment_fresh_for_install, validate_enrollment_request,
    validate_rebind_fresh, validate_rebind_request, validate_request,
    validate_request_fresh_for_signing,
};
use crate::policy_contract::{
    POLICY_RESPONSE_SCHEMA, PolicySignerRequest, PolicySignerResponse, policy_request_root,
    validate_policy_request, validate_policy_request_fresh,
};

pub trait Approval {
    fn now(&self) -> chrono::DateTime<Utc>;
    fn ensure_session(&self, request: &SignerRequest) -> Result<(), String>;
    fn approve(&self, request: &SignerRequest) -> Result<bool, String>;
    fn record_session_use(&self, request: &SignerRequest, key: &SigningKey) -> Result<(), String>;
    fn reauthenticate(&self, request: &SignerRequest) -> Result<(), String>;
    fn ensure_policy_session(&self, _request: &PolicySignerRequest) -> Result<(), String> {
        Err("policy approval sessions are unsupported by this approval provider".to_string())
    }
    fn approve_policy(&self, _request: &PolicySignerRequest) -> Result<bool, String> {
        Err("policy approval cards are unsupported by this approval provider".to_string())
    }
    fn record_policy_session_use(
        &self,
        _request: &PolicySignerRequest,
        _key: &SigningKey,
    ) -> Result<(), String> {
        Err("policy approval sessions are unsupported by this approval provider".to_string())
    }
    fn reauthenticate_policy(&self, _request: &PolicySignerRequest) -> Result<(), String> {
        Err("policy approvals are unsupported by this approval provider".to_string())
    }
    fn reauthenticate_enrollment(&self, request: &EnrollmentRequest) -> Result<(), String>;
    fn reauthenticate_rebind(&self, request: &RebindRequest) -> Result<(), String>;
    fn record_enrollment_session(
        &self,
        request: &EnrollmentRequest,
        key: &SigningKey,
    ) -> Result<(), String>;
    fn record_rebind_session(
        &self,
        request: &RebindRequest,
        key: &SigningKey,
    ) -> Result<(), String>;
}

pub fn rebind<A: Approval, C: Custody>(
    request: &RebindRequest,
    approval: &A,
    custody: &C,
    helper_path: &std::path::Path,
    now: chrono::DateTime<Utc>,
) -> Result<RebindResponse, String> {
    validate_rebind_request(request, now)?;
    let helper_sha256 = file_sha256(helper_path)?;
    if helper_sha256 != request.helper_sha256 {
        return Err("running helper digest does not match rebind request".to_string());
    }
    if custody.provider() != request.provider {
        return Err("rebind custody provider does not match helper provider".to_string());
    }
    approval.reauthenticate_rebind(request)?;
    let rebound_at = approval.now();
    validate_rebind_request(request, rebound_at)?;
    validate_rebind_fresh(request, rebound_at)?;

    let mut seed = custody.load_seed(&request.actor, &request.public_key)?;
    let key = SigningKey::from_bytes(&seed);
    seed.zeroize();
    let derived = hex::encode(key.verifying_key().to_bytes());
    if derived != request.public_key {
        return Err("custody seed does not match the rebind public key".to_string());
    }
    if request.protection_mode == ProtectionMode::Session {
        approval.record_rebind_session(request, &key)?;
    }
    let request_root = rebind_request_root(request)?;
    let mut response = RebindResponse {
        schema: REBIND_RESPONSE_SCHEMA.to_string(),
        request_root,
        actor: request.actor.clone(),
        public_key: request.public_key.clone(),
        helper_version: env!("CARGO_PKG_VERSION").to_string(),
        helper_sha256,
        provider: custody.provider().to_string(),
        protection_grade: custody.protection_grade().to_string(),
        protection_mode: request.protection_mode,
        rebound_at: rebound_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        signature: String::new(),
    };
    let signature = key.sign(&rebind_response_signing_bytes(&response)?);
    response.signature = format!("v1:{}", hex::encode(signature.to_bytes()));
    drop(key);
    Ok(response)
}

pub trait Custody {
    fn provider(&self) -> &str;
    fn provider_session(&self) -> Result<String, String>;
    fn protection_grade(&self) -> &str;
    fn load_seed(&self, actor: &str, public_key: &str) -> Result<[u8; 32], String>;
    fn store_seed(&self, actor: &str, public_key: &str, seed: &[u8; 32]) -> Result<(), String>;
    fn delete_seed(&self, actor: &str, public_key: &str) -> Result<(), String>;
}

pub fn enroll<A: Approval, C: Custody>(
    request: &EnrollmentRequest,
    approval: &A,
    custody: &C,
    helper_path: &std::path::Path,
    now: chrono::DateTime<Utc>,
) -> Result<EnrollmentResponse, String> {
    validate_enrollment_request(request, now)?;
    let helper_sha256 = file_sha256(helper_path)?;
    if helper_sha256 != request.helper_sha256 {
        return Err("running helper digest does not match enrollment request".to_string());
    }
    if custody.provider() != request.provider {
        return Err("enrollment custody provider does not match helper provider".to_string());
    }
    // The explicit `vela id protect` invocation is the enrollment request.
    // The platform authenticator is its only human ceremony; a preceding
    // generic Yes/No alert adds fatigue and no independent authorization.
    approval.reauthenticate_enrollment(request)?;
    let installed_at = approval.now();
    validate_enrollment_request(request, installed_at)?;
    validate_enrollment_fresh_for_install(request, installed_at)?;

    let mut source = read_protected_source(&request.source_path)?;
    let key = vela_protocol::sign::signing_key_from_hex(source.trim())?;
    source.zeroize();
    let derived = hex::encode(key.verifying_key().to_bytes());
    if derived != request.public_key {
        return Err("plaintext source key does not match the configured public key".to_string());
    }
    let mut seed = key.to_bytes();
    drop(key);
    if let Err(error) = custody.store_seed(&request.actor, &request.public_key, &seed) {
        seed.zeroize();
        return Err(error);
    }
    seed.zeroize();
    let mut readback = match custody.load_seed(&request.actor, &request.public_key) {
        Ok(seed) => seed,
        Err(error) => {
            let _ = custody.delete_seed(&request.actor, &request.public_key);
            return Err(format!("protected key readback failed: {error}"));
        }
    };
    let readback_key = SigningKey::from_bytes(&readback);
    readback.zeroize();
    let readback_public = hex::encode(readback_key.verifying_key().to_bytes());
    if readback_public != request.public_key {
        let _ = custody.delete_seed(&request.actor, &request.public_key);
        return Err("protected key readback derived the wrong public key".to_string());
    }
    if request.protection_mode == crate::contract::ProtectionMode::Session
        && let Err(error) = approval.record_enrollment_session(request, &readback_key)
    {
        drop(readback_key);
        let _ = custody.delete_seed(&request.actor, &request.public_key);
        return Err(error);
    }
    drop(readback_key);

    Ok(EnrollmentResponse {
        schema: ENROLLMENT_RESPONSE_SCHEMA.to_string(),
        nonce: request.nonce.clone(),
        helper_version: env!("CARGO_PKG_VERSION").to_string(),
        vela_binary_sha256: request.vela_binary_sha256.clone(),
        helper_sha256,
        actor: request.actor.clone(),
        public_key: request.public_key.clone(),
        key_id: format!("{}:{}", request.actor, request.public_key),
        provider: custody.provider().to_string(),
        protection_grade: custody.protection_grade().to_string(),
        protection_mode: request.protection_mode,
        installed_at: installed_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        source_removed: false,
    })
}

fn read_protected_source(path: &str) -> Result<String, String> {
    let path = std::path::Path::new(path);
    let linked = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reinspect plaintext source key: {error}"))?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err("plaintext source key must remain a regular non-symlink file".to_string());
    }
    let inspected = same_file::Handle::from_path(path)
        .map_err(|error| format!("identify plaintext source key: {error}"))?;
    let file =
        std::fs::File::open(path).map_err(|error| format!("open plaintext source key: {error}"))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone plaintext source descriptor: {error}"))?,
    )
    .map_err(|error| format!("identify open plaintext source key: {error}"))?;
    if inspected != opened {
        return Err("plaintext source key changed while it was opened".to_string());
    }
    let mut source = String::new();
    file.take(130)
        .read_to_string(&mut source)
        .map_err(|error| format!("read plaintext source key: {error}"))?;
    if source.len() > 129 {
        source.zeroize();
        return Err("plaintext source key is unexpectedly large".to_string());
    }
    let final_link = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reinspect plaintext source after read: {error}"))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("reidentify plaintext source after read: {error}"))?;
    if final_link.file_type().is_symlink() || !final_link.is_file() || opened != final_identity {
        source.zeroize();
        return Err("plaintext source key changed while it was read".to_string());
    }
    Ok(source)
}

pub fn approve_and_sign<A: Approval, C: Custody>(
    request: &SignerRequest,
    approval: &A,
    custody: &C,
    helper_path: &std::path::Path,
    now: chrono::DateTime<Utc>,
) -> Result<SignerResponse, String> {
    validate_request(request, now)?;
    let helper_sha256 = file_sha256(helper_path)?;
    if helper_sha256 != request.helper_sha256 {
        return Err("running helper digest does not match request".to_string());
    }
    if custody.provider() != request.provider {
        return Err("requested custody provider does not match helper provider".to_string());
    }
    if !approval.approve(request)? {
        return Err("decision declined or cancelled".to_string());
    }
    match request.protection_mode {
        crate::contract::ProtectionMode::Session => approval.ensure_session(request)?,
        crate::contract::ProtectionMode::Always => approval.reauthenticate(request)?,
    }
    let approved_at = approval.now();
    validate_request_fresh_for_signing(request, approved_at)?;

    let mut seed = custody.load_seed(&request.reviewer_actor, &request.reviewer_public_key)?;
    let key = SigningKey::from_bytes(&seed);
    seed.zeroize();
    let public_key = hex::encode(key.verifying_key().to_bytes());
    if public_key != request.reviewer_public_key {
        return Err("custody seed does not match the requested reviewer key".to_string());
    }
    let signatures = request
        .events
        .iter()
        .map(|item| {
            vela_protocol::sign::sign_event(&item.event, &key).map(|signature| EventSignature {
                event_id: item.event.id.clone(),
                signature,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if request.protection_mode == crate::contract::ProtectionMode::Session {
        approval.record_session_use(request, &key)?;
    }
    drop(key);

    Ok(SignerResponse {
        schema: RESPONSE_SCHEMA.to_string(),
        request_root: request_root(request)?,
        reviewer_public_key: public_key,
        helper_version: env!("CARGO_PKG_VERSION").to_string(),
        helper_sha256,
        provider: custody.provider().to_string(),
        protection_grade: custody.protection_grade().to_string(),
        provider_session: custody.provider_session()?,
        approved_at: approved_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        protection_mode: request.protection_mode,
        signatures,
    })
}

pub fn approve_and_sign_policy<A: Approval, C: Custody>(
    request: &PolicySignerRequest,
    approval: &A,
    custody: &C,
    helper_path: &std::path::Path,
    now: chrono::DateTime<Utc>,
) -> Result<PolicySignerResponse, String> {
    validate_policy_request(request, now)?;
    let helper_sha256 = file_sha256(helper_path)?;
    if helper_sha256 != request.helper_sha256 {
        return Err("running helper digest does not match policy request".to_string());
    }
    if custody.provider() != request.provider {
        return Err("requested policy custody provider does not match helper provider".to_string());
    }
    if !approval.approve_policy(request)? {
        return Err("policy decision declined or cancelled".to_string());
    }
    match request.protection_mode {
        ProtectionMode::Session => approval.ensure_policy_session(request)?,
        ProtectionMode::Always => approval.reauthenticate_policy(request)?,
    }
    let approved_at = approval.now();
    validate_policy_request_fresh(request, approved_at)?;

    let mut seed = custody.load_seed(&request.reviewer_actor, &request.reviewer_public_key)?;
    let key = SigningKey::from_bytes(&seed);
    seed.zeroize();
    let public_key = hex::encode(key.verifying_key().to_bytes());
    if public_key != request.reviewer_public_key {
        return Err("custody seed does not match the requested policy reviewer key".to_string());
    }
    let policy_signature = request
        .action
        .ne(&crate::policy_contract::PolicyDecisionAction::Revoke)
        .then(|| {
            vela_protocol::acceptance_policy::policy_signature_preimage(
                &request.policy,
                &request.observed_at,
            )
            .map(|bytes| hex::encode(key.sign(&bytes).to_bytes()))
        })
        .transpose()?;
    let event_signature = vela_protocol::sign::sign_event(&request.event, &key)?;
    if request.protection_mode == ProtectionMode::Session {
        approval.record_policy_session_use(request, &key)?;
    }
    drop(key);

    Ok(PolicySignerResponse {
        schema: POLICY_RESPONSE_SCHEMA.to_string(),
        request_root: policy_request_root(request)?,
        reviewer_public_key: public_key,
        helper_version: env!("CARGO_PKG_VERSION").to_string(),
        helper_sha256,
        provider: custody.provider().to_string(),
        protection_grade: custody.protection_grade().to_string(),
        provider_session: custody.provider_session()?,
        approved_at: approved_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        protection_mode: request.protection_mode,
        policy_signature,
        event_id: request.event.id.clone(),
        event_signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use std::cell::{Cell, RefCell};
    use std::io::Write;

    struct FakeApproval {
        approved: bool,
        reauths: Cell<usize>,
        recorded_sessions: Cell<usize>,
    }

    impl Approval for FakeApproval {
        fn now(&self) -> chrono::DateTime<Utc> {
            "2026-07-17T12:00:30Z".parse().unwrap()
        }

        fn ensure_session(&self, _request: &SignerRequest) -> Result<(), String> {
            Ok(())
        }

        fn approve(&self, _request: &SignerRequest) -> Result<bool, String> {
            Ok(self.approved)
        }

        fn record_session_use(
            &self,
            _request: &SignerRequest,
            _key: &SigningKey,
        ) -> Result<(), String> {
            Ok(())
        }

        fn reauthenticate(&self, _request: &SignerRequest) -> Result<(), String> {
            self.reauths.set(self.reauths.get() + 1);
            Ok(())
        }

        fn reauthenticate_enrollment(&self, _request: &EnrollmentRequest) -> Result<(), String> {
            self.reauths.set(self.reauths.get() + 1);
            Ok(())
        }

        fn reauthenticate_rebind(&self, _request: &RebindRequest) -> Result<(), String> {
            self.reauths.set(self.reauths.get() + 1);
            Ok(())
        }

        fn record_enrollment_session(
            &self,
            _request: &EnrollmentRequest,
            _key: &SigningKey,
        ) -> Result<(), String> {
            self.recorded_sessions.set(self.recorded_sessions.get() + 1);
            Ok(())
        }

        fn record_rebind_session(
            &self,
            _request: &RebindRequest,
            _key: &SigningKey,
        ) -> Result<(), String> {
            self.recorded_sessions.set(self.recorded_sessions.get() + 1);
            Ok(())
        }
    }

    struct FakeCustody {
        seed: [u8; 32],
    }

    impl Custody for FakeCustody {
        fn provider(&self) -> &str {
            "test"
        }

        fn provider_session(&self) -> Result<String, String> {
            Ok("test_session".to_string())
        }

        fn protection_grade(&self) -> &str {
            "user_session"
        }

        fn load_seed(&self, _actor: &str, _public_key: &str) -> Result<[u8; 32], String> {
            Ok(self.seed)
        }

        fn store_seed(
            &self,
            _actor: &str,
            _public_key: &str,
            _seed: &[u8; 32],
        ) -> Result<(), String> {
            Ok(())
        }
        fn delete_seed(&self, _actor: &str, _public_key: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn fixture() -> (
        tempfile::NamedTempFile,
        tempfile::NamedTempFile,
        SignerRequest,
        [u8; 32],
    ) {
        let mut vela = tempfile::NamedTempFile::new().unwrap();
        vela.write_all(b"vela").unwrap();
        let mut helper = tempfile::NamedTempFile::new().unwrap();
        helper.write_all(b"helper").unwrap();
        let seed = [7_u8; 32];
        let public_key = hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes());
        let decision_root = format!("sha256:{}", "4".repeat(64));
        let mut event = vela_protocol::events::new_review_decision_event(
            "vpr_fixture",
            "finding.add",
            "rejected",
            None,
            "reviewer:fixture",
            "not enough evidence",
            Some("2026-07-17T12:00:00Z"),
        )
        .unwrap();
        let mut provenance = vela_protocol::provenance::Provenance::default();
        provenance.bind_decision_root(&decision_root).unwrap();
        vela_protocol::provenance::attach_to_payload(&mut event.payload, &provenance).unwrap();
        event.id = vela_protocol::events::compute_event_id(&event);
        let request = SignerRequest {
            schema: crate::contract::REQUEST_SCHEMA.to_string(),
            nonce: "1".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: vela.path().display().to_string(),
            vela_binary_sha256: file_sha256(vela.path()).unwrap(),
            helper_sha256: file_sha256(helper.path()).unwrap(),
            frontier_id: "vfr_fixture".to_string(),
            frontier_path: "/tmp/frontier".to_string(),
            proposal_id: "vpr_fixture".to_string(),
            proposal_root: format!("sha256:{}", "3".repeat(64)),
            action: "reject".to_string(),
            reason: "not enough evidence".to_string(),
            reviewer_actor: "reviewer:fixture".to_string(),
            reviewer_public_key: public_key,
            observed_at: "2026-07-17T12:00:00Z".to_string(),
            decision_plan_root: decision_root,
            gate_state: "accept_blocked_reject_available".to_string(),
            provider: "test".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: crate::contract::ProtectionMode::Session,
            display: crate::contract::SignerDisplay {
                frontier_name: "Fixture frontier".to_string(),
                claim: "A bounded fixture result".to_string(),
                requester: "agent:fixture".to_string(),
                decisive_facts: vec!["No independent verifier evidence".to_string()],
                consequence: "Keep accepted state unchanged and close this proposal".to_string(),
            },
            events: vec![crate::contract::SignerEvent { event }],
        };
        (vela, helper, request, seed)
    }

    fn enrollment_fixture(
        seed: [u8; 32],
    ) -> (
        tempfile::NamedTempFile,
        tempfile::NamedTempFile,
        tempfile::NamedTempFile,
        EnrollmentRequest,
    ) {
        let mut vela = tempfile::NamedTempFile::new().unwrap();
        vela.write_all(b"vela").unwrap();
        let mut helper = tempfile::NamedTempFile::new().unwrap();
        helper.write_all(b"helper").unwrap();
        let mut source = tempfile::NamedTempFile::new().unwrap();
        source.write_all(hex::encode(seed).as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(source.path(), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        let public_key = hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes());
        let request = EnrollmentRequest {
            schema: crate::contract::ENROLLMENT_REQUEST_SCHEMA.to_string(),
            nonce: "2".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: vela.path().display().to_string(),
            vela_binary_sha256: file_sha256(vela.path()).unwrap(),
            helper_sha256: file_sha256(helper.path()).unwrap(),
            actor: "reviewer:fixture".to_string(),
            public_key,
            source_path: source.path().display().to_string(),
            provider: "test".to_string(),
            protection_mode: crate::contract::ProtectionMode::Session,
            remove_source_after_install: true,
        };
        (vela, helper, source, request)
    }

    #[test]
    fn protected_signer_exact_approval_returns_verifiable_signatures() {
        let (_vela, helper, request, seed) = fixture();
        let approval = FakeApproval {
            approved: true,
            reauths: Cell::new(0),
            recorded_sessions: Cell::new(0),
        };
        let response = approve_and_sign(
            &request,
            &approval,
            &FakeCustody { seed },
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap();
        crate::contract::validate_response(&request, &response).unwrap();
        assert_eq!(approval.reauths.get(), 0);
    }

    #[test]
    fn protected_signer_cancellation_authenticates_nothing_and_reads_no_key() {
        struct CancelApproval;
        impl Approval for CancelApproval {
            fn now(&self) -> chrono::DateTime<Utc> {
                "2026-07-17T12:00:30Z".parse().unwrap()
            }
            fn ensure_session(&self, _request: &SignerRequest) -> Result<(), String> {
                panic!("cancellation must not open or refresh a signer session")
            }
            fn approve(&self, _request: &SignerRequest) -> Result<bool, String> {
                Ok(false)
            }
            fn record_session_use(
                &self,
                _request: &SignerRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                panic!("cancellation must not update a signer session")
            }
            fn reauthenticate(&self, _request: &SignerRequest) -> Result<(), String> {
                panic!("cancellation must not authenticate")
            }
            fn reauthenticate_enrollment(
                &self,
                _request: &EnrollmentRequest,
            ) -> Result<(), String> {
                panic!("decision cancellation must not enter enrollment")
            }
            fn reauthenticate_rebind(&self, _request: &RebindRequest) -> Result<(), String> {
                panic!("decision cancellation must not enter rebind")
            }
            fn record_enrollment_session(
                &self,
                _request: &EnrollmentRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                panic!("decision cancellation must not enter enrollment")
            }
            fn record_rebind_session(
                &self,
                _request: &RebindRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                panic!("decision cancellation must not enter rebind")
            }
        }
        struct PanicCustody;
        impl Custody for PanicCustody {
            fn provider(&self) -> &str {
                "test"
            }
            fn provider_session(&self) -> Result<String, String> {
                Ok("test_session".to_string())
            }
            fn protection_grade(&self) -> &str {
                "user_session"
            }
            fn load_seed(&self, _actor: &str, _public_key: &str) -> Result<[u8; 32], String> {
                panic!("custody must not be read after cancellation")
            }
            fn store_seed(
                &self,
                _actor: &str,
                _public_key: &str,
                _seed: &[u8; 32],
            ) -> Result<(), String> {
                panic!("must not store")
            }
            fn delete_seed(&self, _actor: &str, _public_key: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let (_vela, helper, request, _seed) = fixture();
        let result = approve_and_sign(
            &request,
            &CancelApproval,
            &PanicCustody,
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn protected_signer_always_mode_requires_reauthentication() {
        let (_vela, helper, mut request, seed) = fixture();
        request.protection_mode = crate::contract::ProtectionMode::Always;
        let approval = FakeApproval {
            approved: true,
            reauths: Cell::new(0),
            recorded_sessions: Cell::new(0),
        };
        approve_and_sign(
            &request,
            &approval,
            &FakeCustody { seed },
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(approval.reauths.get(), 1);
    }

    #[test]
    fn protected_signer_rebind_requires_authentication_and_signs_every_response_field() {
        let (vela, helper, request, seed) = fixture();
        let rebind_request = RebindRequest {
            schema: crate::contract::REBIND_REQUEST_SCHEMA.to_string(),
            purpose: crate::contract::RebindPurpose::Upgrade,
            nonce: "8".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: vela.path().display().to_string(),
            vela_binary_sha256: file_sha256(vela.path()).unwrap(),
            previous_vela_binary_sha256: file_sha256(vela.path()).unwrap(),
            helper_sha256: file_sha256(helper.path()).unwrap(),
            previous_helper_sha256: format!("sha256:{}", "9".repeat(64)),
            actor: request.reviewer_actor,
            public_key: request.reviewer_public_key,
            provider: "test".to_string(),
            previous_protection_mode: crate::contract::ProtectionMode::Session,
            protection_mode: crate::contract::ProtectionMode::Session,
        };
        let approval = FakeApproval {
            approved: true,
            reauths: Cell::new(0),
            recorded_sessions: Cell::new(0),
        };
        let response = rebind(
            &rebind_request,
            &approval,
            &FakeCustody { seed },
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(approval.reauths.get(), 1);
        assert_eq!(approval.recorded_sessions.get(), 1);
        crate::contract::validate_rebind_response(&rebind_request, &response).unwrap();

        let mut tampered = response;
        tampered.protection_grade = "file".to_string();
        assert!(crate::contract::validate_rebind_response(&rebind_request, &tampered).is_err());
    }

    #[test]
    fn protected_signer_enrollment_authenticates_stores_and_reads_back() {
        struct EnrollmentCustody {
            stored: RefCell<Option<[u8; 32]>>,
        }
        impl Custody for EnrollmentCustody {
            fn provider(&self) -> &str {
                "test"
            }
            fn provider_session(&self) -> Result<String, String> {
                Ok("test_session".to_string())
            }
            fn protection_grade(&self) -> &str {
                "user_session"
            }
            fn load_seed(&self, _actor: &str, _public_key: &str) -> Result<[u8; 32], String> {
                self.stored
                    .borrow()
                    .as_ref()
                    .copied()
                    .ok_or_else(|| "no stored seed".to_string())
            }
            fn store_seed(
                &self,
                _actor: &str,
                _public_key: &str,
                seed: &[u8; 32],
            ) -> Result<(), String> {
                *self.stored.borrow_mut() = Some(*seed);
                Ok(())
            }
            fn delete_seed(&self, _actor: &str, _public_key: &str) -> Result<(), String> {
                *self.stored.borrow_mut() = None;
                Ok(())
            }
        }

        let seed = [9_u8; 32];
        let (_vela, helper, source, request) = enrollment_fixture(seed);
        let approval = FakeApproval {
            approved: true,
            reauths: Cell::new(0),
            recorded_sessions: Cell::new(0),
        };
        let custody = EnrollmentCustody {
            stored: RefCell::new(None),
        };
        let response = enroll(
            &request,
            &approval,
            &custody,
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(approval.reauths.get(), 1);
        assert_eq!(approval.recorded_sessions.get(), 1);
        assert_eq!(custody.stored.borrow().as_ref(), Some(&seed));
        assert!(source.path().exists());
        assert!(!response.source_removed);
        assert_eq!(response.public_key, request.public_key);
    }

    #[test]
    fn protected_signer_enrollment_public_key_mismatch_never_stores() {
        struct PanicStore;
        impl Custody for PanicStore {
            fn provider(&self) -> &str {
                "test"
            }
            fn provider_session(&self) -> Result<String, String> {
                Ok("test_session".to_string())
            }
            fn protection_grade(&self) -> &str {
                "user_session"
            }
            fn load_seed(&self, _actor: &str, _public_key: &str) -> Result<[u8; 32], String> {
                panic!("mismatched source must not read custody")
            }
            fn store_seed(
                &self,
                _actor: &str,
                _public_key: &str,
                _seed: &[u8; 32],
            ) -> Result<(), String> {
                panic!("mismatched source must not store custody")
            }
            fn delete_seed(&self, _actor: &str, _public_key: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let (_vela, helper, _source, mut request) = enrollment_fixture([9_u8; 32]);
        request.public_key = hex::encode(
            SigningKey::from_bytes(&[8_u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        let error = enroll(
            &request,
            &FakeApproval {
                approved: true,
                reauths: Cell::new(0),
                recorded_sessions: Cell::new(0),
            },
            &PanicStore,
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("does not match the configured public key"));
    }

    #[test]
    fn protected_signer_approval_completed_after_expiry_reads_no_key() {
        struct LateApproval;
        impl Approval for LateApproval {
            fn now(&self) -> chrono::DateTime<Utc> {
                "2026-07-17T12:01:01Z".parse().unwrap()
            }
            fn ensure_session(&self, _request: &SignerRequest) -> Result<(), String> {
                Ok(())
            }
            fn approve(&self, _request: &SignerRequest) -> Result<bool, String> {
                Ok(true)
            }
            fn record_session_use(
                &self,
                _request: &SignerRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                panic!("expired request must not update a session")
            }
            fn reauthenticate(&self, _request: &SignerRequest) -> Result<(), String> {
                Ok(())
            }
            fn reauthenticate_enrollment(
                &self,
                _request: &EnrollmentRequest,
            ) -> Result<(), String> {
                Ok(())
            }
            fn reauthenticate_rebind(&self, _request: &RebindRequest) -> Result<(), String> {
                Ok(())
            }
            fn record_enrollment_session(
                &self,
                _request: &EnrollmentRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                Ok(())
            }
            fn record_rebind_session(
                &self,
                _request: &RebindRequest,
                _key: &SigningKey,
            ) -> Result<(), String> {
                Ok(())
            }
        }
        struct PanicCustody;
        impl Custody for PanicCustody {
            fn provider(&self) -> &str {
                "test"
            }
            fn provider_session(&self) -> Result<String, String> {
                Ok("test_session".to_string())
            }
            fn protection_grade(&self) -> &str {
                "user_session"
            }
            fn load_seed(&self, _actor: &str, _public_key: &str) -> Result<[u8; 32], String> {
                panic!("expired request must not read custody")
            }
            fn store_seed(
                &self,
                _actor: &str,
                _public_key: &str,
                _seed: &[u8; 32],
            ) -> Result<(), String> {
                panic!("must not store")
            }
            fn delete_seed(&self, _actor: &str, _public_key: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let (_vela, helper, request, _seed) = fixture();
        let error = approve_and_sign(
            &request,
            &LateApproval,
            &PanicCustody,
            helper.path(),
            "2026-07-17T12:00:00Z".parse().unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("expired before approval completed"));
    }
}
