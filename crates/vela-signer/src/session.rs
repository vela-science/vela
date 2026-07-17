use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};

pub const SESSION_SCHEMA: &str = "vela.signer-session.v1";
pub const SESSION_IDLE_SECONDS: i64 = 15 * 60;
pub const SESSION_OVERALL_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRecord {
    pub schema: String,
    pub actor: String,
    pub public_key: String,
    pub provider: String,
    pub protection_mode: String,
    pub helper_sha256: String,
    pub session_id: String,
    pub authenticated_at: String,
    pub last_used_at: String,
    pub expires_at: String,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Invalid,
    IdleExpired,
    OverallExpired,
}

impl SessionRecord {
    pub fn new(
        actor: &str,
        public_key: &str,
        provider: &str,
        protection_mode: &str,
        helper_sha256: &str,
        now: DateTime<Utc>,
    ) -> Self {
        let mut session_id = [0_u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut session_id);
        Self::new_with_id(
            actor,
            public_key,
            provider,
            protection_mode,
            helper_sha256,
            now,
            hex::encode(session_id),
        )
    }

    fn new_with_id(
        actor: &str,
        public_key: &str,
        provider: &str,
        protection_mode: &str,
        helper_sha256: &str,
        now: DateTime<Utc>,
        session_id: String,
    ) -> Self {
        Self {
            schema: SESSION_SCHEMA.to_string(),
            actor: actor.to_string(),
            public_key: public_key.to_string(),
            provider: provider.to_string(),
            protection_mode: protection_mode.to_string(),
            helper_sha256: helper_sha256.to_string(),
            session_id,
            authenticated_at: timestamp(now),
            last_used_at: timestamp(now),
            expires_at: timestamp(now + Duration::seconds(SESSION_OVERALL_SECONDS)),
            signature: String::new(),
        }
    }

    pub fn state(
        &self,
        actor: &str,
        public_key: &str,
        provider: &str,
        protection_mode: &str,
        helper_sha256: &str,
        now: DateTime<Utc>,
    ) -> SessionState {
        if self.schema != SESSION_SCHEMA
            || self.actor != actor
            || self.public_key != public_key
            || self.provider != provider
            || self.protection_mode != protection_mode
            || self.helper_sha256 != helper_sha256
            || self.session_id.len() != 64
            || !self
                .session_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return SessionState::Invalid;
        }
        if self.verify_signature().is_err() {
            return SessionState::Invalid;
        }
        let Ok(authenticated_at) = parse(&self.authenticated_at) else {
            return SessionState::Invalid;
        };
        let Ok(last_used_at) = parse(&self.last_used_at) else {
            return SessionState::Invalid;
        };
        let Ok(expires_at) = parse(&self.expires_at) else {
            return SessionState::Invalid;
        };
        if authenticated_at > now + Duration::seconds(60)
            || last_used_at < authenticated_at
            || expires_at != authenticated_at + Duration::seconds(SESSION_OVERALL_SECONDS)
        {
            return SessionState::Invalid;
        }
        if now > expires_at {
            return SessionState::OverallExpired;
        }
        if now > last_used_at + Duration::seconds(SESSION_IDLE_SECONDS) {
            return SessionState::IdleExpired;
        }
        SessionState::Active
    }

    pub fn touch(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        let authenticated_at = parse(&self.authenticated_at)?;
        let expires_at = parse(&self.expires_at)?;
        if now > expires_at
            || now > parse(&self.last_used_at)? + Duration::seconds(SESSION_IDLE_SECONDS)
        {
            return Err("signer session expired before completion".to_string());
        }
        if now < authenticated_at - Duration::seconds(60) {
            return Err("signer session clock moved before authentication".to_string());
        }
        self.last_used_at = timestamp(now);
        Ok(())
    }

    pub fn sign(&mut self, key: &SigningKey) -> Result<(), String> {
        let expected = hex::encode(key.verifying_key().to_bytes());
        if expected != self.public_key {
            return Err("signer session key does not match its public key".to_string());
        }
        self.signature.clear();
        let signature = key.sign(&self.signing_bytes()?);
        self.signature = format!("v1:{}", hex::encode(signature.to_bytes()));
        Ok(())
    }

    fn verify_signature(&self) -> Result<(), String> {
        let public = hex::decode(&self.public_key)
            .map_err(|error| format!("invalid signer session public key: {error}"))?;
        let verifying = VerifyingKey::from_bytes(
            &public
                .try_into()
                .map_err(|_| "signer session public key must be 32 bytes".to_string())?,
        )
        .map_err(|error| format!("invalid signer session public key: {error}"))?;
        let raw = self
            .signature
            .strip_prefix("v1:")
            .ok_or_else(|| "signer session signature is missing".to_string())?;
        let bytes = hex::decode(raw)
            .map_err(|error| format!("invalid signer session signature: {error}"))?;
        let signature = Signature::from_bytes(
            &bytes
                .try_into()
                .map_err(|_| "signer session signature must be 64 bytes".to_string())?,
        );
        verifying
            .verify(&self.signing_bytes()?, &signature)
            .map_err(|_| "signer session signature does not verify".to_string())
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        let canonical = vela_protocol::canonical::to_canonical_bytes(&unsigned)
            .map_err(|error| format!("canonicalize signer session: {error}"))?;
        let mut bytes = b"vela.signer-session.v1\0".to_vec();
        bytes.extend(canonical);
        Ok(bytes)
    }
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

fn parse(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| format!("invalid signer session time: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    #[test]
    fn session_has_exact_idle_and_overall_limits() {
        let key = SigningKey::from_bytes(&[4_u8; 32]);
        let public = hex::encode(key.verifying_key().to_bytes());
        let mut record = SessionRecord::new(
            "reviewer:test",
            &public,
            "os_store",
            "session",
            &format!("sha256:{}", "b".repeat(64)),
            at("2026-07-17T12:00:00Z"),
        );
        record.sign(&key).unwrap();
        assert_eq!(
            record.state(
                "reviewer:test",
                &public,
                "os_store",
                "session",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T12:15:00Z")
            ),
            SessionState::Active
        );
        assert_eq!(
            record.state(
                "reviewer:test",
                &public,
                "os_store",
                "session",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T12:15:01Z")
            ),
            SessionState::IdleExpired
        );
        let mut touched = record;
        touched.touch(at("2026-07-17T12:14:00Z")).unwrap();
        touched.sign(&key).unwrap();
        assert_eq!(
            touched.state(
                "reviewer:test",
                &public,
                "os_store",
                "session",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T13:00:01Z")
            ),
            SessionState::OverallExpired
        );
    }

    #[test]
    fn identity_helper_or_provider_drift_invalidates_session() {
        let key = SigningKey::from_bytes(&[5_u8; 32]);
        let public = hex::encode(key.verifying_key().to_bytes());
        let mut record = SessionRecord::new(
            "reviewer:test",
            &public,
            "os_store",
            "session",
            &format!("sha256:{}", "b".repeat(64)),
            at("2026-07-17T12:00:00Z"),
        );
        record.sign(&key).unwrap();
        assert_eq!(
            record.state(
                "reviewer:other",
                &public,
                "os_store",
                "session",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T12:01:00Z")
            ),
            SessionState::Invalid
        );
        assert_eq!(
            record.state(
                "reviewer:test",
                &public,
                "os_store",
                "always",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T12:01:00Z")
            ),
            SessionState::Invalid
        );
    }

    #[test]
    fn timestamp_or_binding_tampering_invalidates_session_signature() {
        let key = SigningKey::from_bytes(&[6_u8; 32]);
        let public = hex::encode(key.verifying_key().to_bytes());
        let mut record = SessionRecord::new(
            "reviewer:test",
            &public,
            "os_store",
            "session",
            &format!("sha256:{}", "b".repeat(64)),
            at("2026-07-17T12:00:00Z"),
        );
        record.sign(&key).unwrap();
        record.last_used_at = "2026-07-17T12:10:00.000000000Z".to_string();
        assert_eq!(
            record.state(
                "reviewer:test",
                &public,
                "os_store",
                "session",
                &format!("sha256:{}", "b".repeat(64)),
                at("2026-07-17T12:11:00Z")
            ),
            SessionState::Invalid
        );
    }
}
