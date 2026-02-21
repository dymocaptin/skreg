//! Certificate revocation checking.

use std::collections::HashSet;

use crate::error::RevocationError;

/// Checks whether a certificate serial number has been revoked.
pub trait RevocationStore: Send + Sync {
    /// Return `true` if the given serial number appears in the revocation list.
    ///
    /// # Errors
    ///
    /// Returns [`RevocationError`] if the store cannot be queried.
    fn is_revoked(&self, cert_serial: u64) -> Result<bool, RevocationError>;
}

/// An in-memory [`RevocationStore`] for use in tests and offline scenarios.
#[derive(Debug, Default)]
pub struct InMemoryRevocationStore {
    revoked: HashSet<u64>,
}

impl InMemoryRevocationStore {
    /// Create an empty revocation store.
    pub fn new() -> Self {
        Self { revoked: HashSet::new() }
    }

    /// Mark a certificate serial as revoked.
    pub fn revoke(&mut self, serial: u64) {
        self.revoked.insert(serial);
    }
}

impl RevocationStore for InMemoryRevocationStore {
    fn is_revoked(&self, cert_serial: u64) -> Result<bool, RevocationError> {
        Ok(self.revoked.contains(&cert_serial))
    }
}
