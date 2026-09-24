//! Multi-account Google integration tests: bridge add/remove bookkeeping,
//! per-account token routing, and source resolution. Credential
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

/// A bridge credential whose token is far from expiry, so `access_token` never
/// calls the network.
fn sample_credential(access: &str) -> tokens::BridgeCredential {
    tokens::BridgeCredential {
        bridge_url: format!("https://script.google.com/macros/s/{access}/exec"),
        secret: "s3cret".to_string(),
        deployment_id: access.to_string(),
        access_token: access.to_string(),
        expires_at: tokens::expiry_from_now(3600),
    }
}

/// Adds an account directly (bypassing the bridge call) by writing its
/// credential and source row exactly as `add_bridge` would on success.
fn seed_account(conn: &rusqlite::Connection, email: &str, access: &str) -> String {
    let account_key = account_key_from_email(email);
    tokens::save(&account_key, &sample_credential(access)).expect("save credential");
    upsert_account_source(conn, &source_id_for(&account_key), email).expect("upsert source");
    source_id_for(&account_key)
}

/// Writes a raw keychain value for an account, for credentials in old shapes.
fn save_raw(account_key: &str, raw: &str) {
    keyring::Entry::new(
        crate::vault::KEYRING_SERVICE,
        &tokens::keyring_user_for(account_key),
    )
    .expect("entry")
    .set_password(raw)
    .expect("write raw");
}

/// A syntactically valid bare spreadsheet id for routing tests.
const SPREADSHEET_ID: &str = "1AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-abc";

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
    assert_eq!(account_key_from_source_id("google-sheets"), None);
    assert_eq!(account_key_from_source_id("mock-source"), None);
}

#[test]
fn two_accounts_coexist_with_independent_tokens_and_rows() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let alice_id = seed_account(&conn, "alice@example.com", "alice-token");
    let bob_id = seed_account(&conn, "bob@example.com", "bob-token");

    let accounts = list_accounts(&conn).expect("list");
    assert_eq!(accounts.len(), 2);
    assert_eq!(
        accounts[0],
        GoogleAccount {
            source_id: alice_id.clone(),
            email: "alice@example.com".to_string(),
            deployment_id: "alice-token".to_string(),
            bridge_url: "https://script.google.com/macros/s/alice-token/exec".to_string(),
        }
    );
    assert_eq!(accounts[1].email, "bob@example.com");
    assert!(has_any_account(&conn).expect("any"));

    assert_eq!(
        access_token(&conn, &alice_id).expect("alice"),
        "alice-token"
    );
    assert_eq!(access_token(&conn, &bob_id).expect("bob"), "bob-token");

    remove_bridge(&conn, &alice_id).expect("remove alice");
    let remaining = list_accounts(&conn).expect("list after remove");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].source_id, bob_id);
    assert_eq!(
        access_token(&conn, &bob_id).expect("bob still works"),
        "bob-token"
    );
    let error = access_token(&conn, &alice_id).expect_err("alice gone");
    assert_eq!(error.to_string(), NOT_CONNECTED_MESSAGE);

    remove_bridge(&conn, &bob_id).expect("cleanup bob");
}

#[test]
fn re_adding_the_same_email_replaces_the_credential() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let id = seed_account(&conn, "carol@example.com", "old-token");
    let id_again = seed_account(&conn, "carol@example.com", "new-token");
    assert_eq!(id, id_again);

    assert_eq!(list_accounts(&conn).expect("list").len(), 1);
    assert_eq!(access_token(&conn, &id).expect("token"), "new-token");

    remove_bridge(&conn, &id).expect("cleanup");
}

#[test]
fn an_old_oauth_credential_is_listed_for_re_adding() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let id = seed_account(&conn, "erin@example.com", "erin-token");
    save_raw(
        "erin_example_com",
        r#"{"accessToken":"a","refreshToken":"r","expiresAt":"2026-01-01T00:00:00.000Z"}"#,
    );

    let accounts = list_accounts(&conn).expect("list");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email, "erin@example.com");
    assert_eq!(accounts[0].deployment_id, "");
    assert_eq!(accounts[0].bridge_url, "");

    let error = access_token(&conn, &id).expect_err("stale");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
    let error = test_bridge(&conn, &id).expect_err("stale");
    assert!(matches!(error, CoreError::PermissionDenied(_)));

    remove_bridge(&conn, &id).expect("cleanup");
}

#[test]
fn remove_bridge_is_idempotent_and_clears_its_routes() {
    let _guard = keychain_guard();
    let conn = open_temp_db();

    let frank = seed_account(&conn, "frank@example.com", "frank-token");
    let gina = seed_account(&conn, "gina@example.com", "gina-token");
    db::set_meta(&conn, "google_route:sheetA", &frank).expect("route a");
    db::set_meta(&conn, "google_route:sheetB", &gina).expect("route b");
    for (id, source) in [("item-frank", &frank), ("item-gina", &gina)] {
        conn.execute(
            "INSERT INTO workbench_items (id, folder_id, source_id, spreadsheet_id, name, position)
             VALUES (?1, NULL, ?2, 'sheet', 'Sheet', 0)",
            rusqlite::params![id, source],
        )
        .expect("workbench item");
    }

    remove_bridge(&conn, &frank).expect("first remove");
    remove_bridge(&conn, &frank).expect("second remove is a no-op");

    assert_eq!(db::get_meta(&conn, "google_route:sheetA").expect("a"), None);
    assert_eq!(
        db::get_meta(&conn, "google_route:sheetB").expect("b"),
        Some(gina.clone())
    );
    assert!(matches!(
        test_bridge(&conn, &frank),
        Err(CoreError::NotFound(_))
    ));
    let remaining: Vec<String> = conn
        .prepare("SELECT id FROM workbench_items ORDER BY id")
        .expect("prepare")
        .query_map([], |row| row.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert_eq!(remaining, vec!["item-gina".to_string()]);

    remove_bridge(&conn, &gina).expect("cleanup");
}

#[test]
fn remove_bridge_rejects_a_non_google_source_id() {
    let _guard = keychain_guard();
    let conn = open_temp_db();
    let error = remove_bridge(&conn, "mock-source").expect_err("must reject");
    assert!(matches!(error, CoreError::InvalidInput(_)));
}

#[test]
fn resolve_source_returns_an_explicit_id_unchanged() {
    let conn = open_temp_db();
    assert_eq!(
        resolve_source(&conn, Some("mock-source"), Some("anything")).expect("explicit"),
        "mock-source"
    );
}

#[test]
fn resolve_source_without_accounts_reports_not_connected() {
    let _guard = keychain_guard();
    let conn = open_temp_db();
    let error = resolve_source(&conn, None, Some(SPREADSHEET_ID)).expect_err("none");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
    assert_eq!(error.to_string(), NOT_CONNECTED_MESSAGE);
}

#[test]
fn resolve_source_with_one_account_picks_it() {
    let _guard = keychain_guard();
    let conn = open_temp_db();
    let id = seed_account(&conn, "hank@example.com", "hank-token");

    assert_eq!(resolve_source(&conn, None, None).expect("no table"), id);
    assert_eq!(
        resolve_source(&conn, None, Some(SPREADSHEET_ID)).expect("with table"),
        id
    );

    remove_bridge(&conn, &id).expect("cleanup");
}

#[test]
fn resolve_source_with_several_accounts_uses_first_or_cached_route() {
    let _guard = keychain_guard();
    let conn = open_temp_db();
    let ivy = seed_account(&conn, "ivy@example.com", "ivy-token");
    let jack = seed_account(&conn, "jack@example.com", "jack-token");

    // No table id: first account in list order.
    assert_eq!(resolve_source(&conn, None, None).expect("first"), ivy);

    // A cached route to a live account wins without probing the network.
    db::set_meta(&conn, &format!("google_route:{SPREADSHEET_ID}"), &jack).expect("route");
    assert_eq!(
        resolve_source(&conn, None, Some(SPREADSHEET_ID)).expect("cached"),
        jack
    );

    remove_bridge(&conn, &ivy).expect("cleanup ivy");
    remove_bridge(&conn, &jack).expect("cleanup jack");
}

#[test]
fn default_rule_is_created_once_and_never_overwritten() {
    let conn = open_temp_db();
    let source = "google-sheets:kim_example_com";

    ensure_default_rule(&conn, source).expect("create");
    let rule = permissions::find_rule(&conn, source, None)
        .expect("find")
        .expect("rule exists");
    assert!(rule.read && rule.write && rule.delete_records);

    // A user edit survives a later re-add.
    permissions::save_rule(
        &conn,
        &SavePermissionRule {
            id: Some(rule.id),
            source_id: source.to_string(),
            table_id: None,
            read: true,
            write: false,
            delete_records: false,
        },
    )
    .expect("edit");
    ensure_default_rule(&conn, source).expect("no-op");
    let after = permissions::find_rule(&conn, source, None)
        .expect("find")
        .expect("rule exists");
    assert!(!after.write);
    let count = permissions::list_rules(&conn)
        .expect("list")
        .iter()
        .filter(|rule| rule.source_id == source)
        .count();
    assert_eq!(count, 1);
}
