//! Multi-account Google integration tests: connect/disconnect bookkeeping,
//! per-account token routing, and the legacy single-account migration. Token
//! storage runs against an in-memory credential store installed once for the
//! whole test binary, so these never touch the real OS keychain. The store is
//! shared across the binary, so tests here use distinct account keys and clean
//! up after themselves to stay isolated.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, Once, OnceLock};

use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence};
use keyring::error::Error as KeyringError;

use super::*;
use crate::db::test_support::open_temp_db;
use crate::sources;

/// Process-wide in-memory keychain: `(service, user) -> secret`. Unlike the
/// keyring crate's own mock (which gives every `Entry::new` an independent,
/// empty credential), this persists writes so `save` then `load` round-trips.
fn store() -> &'static Mutex<HashMap<(String, String), String>> {
    static STORE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
struct SharedCredential {
    service: String,
    user: String,
}

impl CredentialApi for SharedCredential {
    fn set_password(&self, password: &str) -> keyring::Result<()> {
        store().lock().unwrap().insert(
            (self.service.clone(), self.user.clone()),
            password.to_string(),
        );
        Ok(())
    }

    fn get_password(&self) -> keyring::Result<String> {
        store()
            .lock()
            .unwrap()
            .get(&(self.service.clone(), self.user.clone()))
            .cloned()
            .ok_or(KeyringError::NoEntry)
    }

    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        self.set_password(&String::from_utf8_lossy(secret))
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.get_password().map(String::into_bytes)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        store()
            .lock()
            .unwrap()
            .remove(&(self.service.clone(), self.user.clone()));
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct SharedBuilder;

impl CredentialBuilderApi for SharedBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(SharedCredential {
            service: service.to_string(),
            user: user.to_string(),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

/// Installs the shared in-memory credential store exactly once per test binary.
fn install_shared_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        keyring::set_default_credential_builder(Box::new(SharedBuilder));
    });
}

/// Serializes keychain-touching tests: the store is a single process-wide map,
/// so parallel writers of the same key would race.
fn keychain_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    install_shared_keyring();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn sample_tokens(access: &str) -> tokens::TokenSet {
    tokens::TokenSet {
        access_token: access.to_string(),
        refresh_token: Some(format!("{access}-refresh")),
        // Far future so access_token never triggers a network refresh.
        expires_at: tokens::expiry_from_now(3600),
    }
}

/// Connects an account directly (bypassing the browser OAuth flow) by writing
/// its tokens and source row exactly as `connect` would on success.
fn seed_account(conn: &rusqlite::Connection, email: &str, access: &str) -> String {
    let account_key = account_key_from_email(email);
    tokens::save(&account_key, &sample_tokens(access)).expect("save tokens");
    sources::upsert(
        conn,
        &source_id_for(&account_key),
        SourceKind::GoogleSheets,
        &format!("Google Sheets ({email})"),
        sources::SOURCE_STATUS_CONNECTED,
    )
    .expect("upsert source");
    source_id_for(&account_key)
}

#[test]
fn account_key_sanitizes_emails_and_falls_back_to_default() {
    assert_eq!(
        account_key_from_email("Alice@Example.com"),
        "alice_example_com"
    );
    assert_eq!(account_key_from_email("  bob@corp.io "), "bob_corp_io");
    // No alphanumerics at all -> the default key rather than an empty suffix.
    assert_eq!(account_key_from_email("@@@"), "default");
}

#[test]
fn source_id_round_trips_through_the_account_key() {
    let id = source_id_for("alice_example_com");
    assert_eq!(id, "google-sheets:alice_example_com");
    assert_eq!(account_key_from_source_id(&id), Some("alice_example_com"));
    // The bare legacy id has no key; foreign ids are rejected.
    assert_eq!(account_key_from_source_id("google-sheets"), None);
    assert_eq!(account_key_from_source_id("mock-source"), None);
}

#[test]
fn two_accounts_coexist_with_independent_tokens_and_rows() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let alice_id = seed_account(&conn, "alice@example.com", "alice-token");
    let bob_id = seed_account(&conn, "bob@example.com", "bob-token");

    // Both appear in the account list, ordered by source id.
    let accounts = list_accounts(&conn).expect("list");
    let emails: Vec<&str> = accounts.iter().map(|a| a.email.as_str()).collect();
    assert_eq!(emails, ["alice@example.com", "bob@example.com"]);
    assert_eq!(accounts.len(), 2);
    assert!(has_any_account(&conn).expect("any"));

    // Each source resolves its OWN access token (no refresh: far-future expiry).
    assert_eq!(
        access_token(&conn, &alice_id).expect("alice"),
        "alice-token"
    );
    assert_eq!(access_token(&conn, &bob_id).expect("bob"), "bob-token");

    // Disconnecting one leaves the other fully intact.
    disconnect(&conn, &alice_id).expect("disconnect alice");
    let remaining = list_accounts(&conn).expect("list after disconnect");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].source_id, bob_id);
    assert_eq!(
        access_token(&conn, &bob_id).expect("bob still works"),
        "bob-token"
    );
    // Alice's token is gone: her source now reads as not connected.
    let error = access_token(&conn, &alice_id).expect_err("alice gone");
    assert!(matches!(error, CoreError::PermissionDenied(_)));

    disconnect(&conn, &bob_id).expect("cleanup bob");
}

#[test]
fn reconnecting_the_same_email_updates_in_place() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let id = seed_account(&conn, "carol@example.com", "old-token");
    // "Reconnect": same email -> same key -> tokens overwritten, one row.
    let id_again = seed_account(&conn, "carol@example.com", "new-token");
    assert_eq!(id, id_again);

    assert_eq!(list_accounts(&conn).expect("list").len(), 1);
    assert_eq!(access_token(&conn, &id).expect("token"), "new-token");

    disconnect(&conn, &id).expect("cleanup");
}

#[test]
fn disconnect_rejects_a_non_google_source_id() {
    let _guard = keychain_guard();
    let conn = open_temp_db();
    let error = disconnect(&conn, "mock-source").expect_err("must reject");
    assert!(matches!(error, CoreError::InvalidInput(_)));
}

#[test]
fn migration_moves_a_legacy_connection_into_the_keyed_scheme() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    // Arrange a pre-multi-account world: a bare "google-sheets" source row plus
    // a legacy single-account keychain entry.
    sources::upsert(
        &conn,
        GOOGLE_SOURCE_ID,
        SourceKind::GoogleSheets,
        "Google Sheets (dave@example.com)",
        sources::SOURCE_STATUS_CONNECTED,
    )
    .expect("legacy source");
    tokens::save_legacy_for_test(&sample_tokens("dave-token"));

    migrate_legacy_account(&conn).expect("migrate");

    // The bare row is gone; a single keyed account remains, correctly labelled.
    let accounts = list_accounts(&conn).expect("list");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email, "dave@example.com");
    assert_eq!(accounts[0].source_id, "google-sheets:dave_example_com");
    assert!(sources::get_kind(&conn, GOOGLE_SOURCE_ID)
        .expect("kind")
        .is_none());

    // The token moved to the keyed entry and is reachable per-account; the
    // legacy entry is cleared so a second migration is a no-op.
    assert_eq!(
        access_token(&conn, &accounts[0].source_id).expect("token"),
        "dave-token"
    );
    assert!(tokens::load_legacy().expect("legacy gone").is_none());
    migrate_legacy_account(&conn).expect("idempotent");
    assert_eq!(list_accounts(&conn).expect("still one").len(), 1);

    disconnect(&conn, &accounts[0].source_id).expect("cleanup");
}
