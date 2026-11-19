//! PKCE (RFC 7636, S256) and OAuth `state` generation (spec §9: 64-byte verifier,
//! 32-byte state, both from the OS CSPRNG).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

/// One login attempt's secrets. Never logged or sent to the UI.
#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    /// 86 base64url chars (64 random bytes).
    pub verifier: String,
    /// base64url(SHA-256(verifier)).
    pub challenge: String,
    /// 43 base64url chars (32 random bytes), checked on the callback.
    pub state: String,
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pkce { .. }")
    }
}

fn random_b64<const N: usize>() -> AppResult<String> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| AppError::internal(format!("rng: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// S256 challenge for a verifier.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

impl Pkce {
    pub fn generate() -> AppResult<Self> {
        let verifier = random_b64::<64>()?;
        Ok(Self {
            challenge: challenge_for(&verifier),
            verifier,
            state: random_b64::<32>()?,
        })
    }
}

/// Constant-time comparison for the returned `state`.
pub fn state_matches(expected: &str, received: &str) -> bool {
    let (a, b) = (expected.as_bytes(), received.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_test_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_values_have_spec_lengths_and_charset() {
        let p = Pkce::generate().unwrap();
        assert_eq!(p.verifier.len(), 86);
        assert_eq!(p.state.len(), 43);
        assert!((43..=128).contains(&p.verifier.len()));
        let ok = |s: &str| s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        assert!(ok(&p.verifier) && ok(&p.state) && ok(&p.challenge));
        assert_eq!(p.challenge, challenge_for(&p.verifier));
    }

    #[test]
    fn values_are_unique() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
    }

    #[test]
    fn state_comparison() {
        assert!(state_matches("abc", "abc"));
        assert!(!state_matches("abc", "abd"));
        assert!(!state_matches("abc", "abcd"));
    }

    #[test]
    fn debug_hides_secrets() {
        let p = Pkce::generate().unwrap();
        assert!(!format!("{p:?}").contains(&p.verifier));
    }
}
