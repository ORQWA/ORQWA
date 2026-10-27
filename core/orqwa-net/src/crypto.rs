//! The existing rustls crypto provider supplies entropy and SHA-256; no homemade randomness.

use std::sync::{Arc, OnceLock};

use rustls::crypto::CryptoProvider;

use crate::code::{AdmissionSecret, Fingerprint, RideId, RouteHash};

/// The operating system's secure random source could not supply bytes.
#[derive(Debug, thiserror::Error)]
#[error("Secure random numbers are unavailable.")]
pub struct RandomError;

pub(crate) fn provider() -> Arc<CryptoProvider> {
    static PROVIDER: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    PROVIDER
        .get_or_init(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
        .clone()
}

/// Fills an array with fresh bytes from the crypto provider's secure random source.
///
/// # Errors
/// Returns [`RandomError`] rather than falling back to a non-cryptographic generator.
pub fn random<const N: usize>() -> Result<[u8; N], RandomError> {
    let mut bytes = [0; N];
    provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| RandomError)?;
    Ok(bytes)
}

/// SHA-256 of a per-ride certificate's complete DER encoding.
#[must_use]
pub fn fingerprint(certificate: &[u8]) -> Fingerprint {
    Fingerprint(sha256(certificate))
}

/// SHA-256 of the common route representation; the CLI uses the original GPX file's bytes.
#[must_use]
pub fn route_hash(route: &[u8]) -> RouteHash {
    RouteHash(sha256(route))
}

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    let provider = provider();
    let hash = provider
        .cipher_suites
        .iter()
        .filter_map(rustls::SupportedCipherSuite::tls13)
        .map(|suite| suite.common.hash_provider)
        .find(|hash| hash.algorithm() == rustls::crypto::hash::HashAlgorithm::SHA256)
        .expect("the fixed aws-lc provider includes TLS 1.3 SHA-256");
    hash.hash(bytes)
        .as_ref()
        .try_into()
        .expect("SHA-256 has 32 bytes")
}

impl RideId {
    /// A fresh ride identifier, never reused across host lifetimes.
    ///
    /// # Errors
    /// Returns [`RandomError`] if secure entropy is unavailable.
    pub fn generate() -> Result<Self, RandomError> {
        let bytes = random()?;
        if bytes == [0; 16] {
            return Err(RandomError);
        }
        Ok(Self(bytes))
    }
}

impl AdmissionSecret {
    /// A fresh 256-bit bearer credential. Never log the resulting reply or publish it.
    ///
    /// # Errors
    /// Returns [`RandomError`] if secure entropy is unavailable.
    pub fn generate() -> Result<Self, RandomError> {
        Self::from_random_bytes(random()?).map_err(|_| RandomError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_sha256_reference_vectors() {
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
    }

    #[test]
    fn separate_sessions_and_replies_get_separate_identifiers_and_credentials() {
        assert_ne!(RideId::generate().unwrap(), RideId::generate().unwrap());
        assert_ne!(
            AdmissionSecret::generate().unwrap(),
            AdmissionSecret::generate().unwrap()
        );
    }
}
