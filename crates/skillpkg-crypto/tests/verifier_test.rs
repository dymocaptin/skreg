use skillpkg_crypto::revocation::{InMemoryRevocationStore, RevocationStore};

#[test]
fn in_memory_store_starts_empty() {
    let store = InMemoryRevocationStore::new();
    assert!(!store.is_revoked(1).unwrap());
}

#[test]
fn in_memory_store_tracks_revoked_serials() {
    let mut store = InMemoryRevocationStore::new();
    store.revoke(42);
    assert!(store.is_revoked(42).unwrap());
    assert!(!store.is_revoked(1).unwrap());
}
