//! TEST DATA. Unit tests inject the environment/keychain boundary.
use std::sync::Arc;
use vellum_contracts::{fakes::MemorySecrets, suites};
use vellum_core::ports::*;
use vellum_secrets_keyring::{EnvSecrets, FallbackSecrets, Keychain};

#[test]
fn environment_is_namespaced_read_only_and_missing_is_explicit() {
    let env = EnvSecrets::with_lookup(|name| {
        (name == "VELLUM_OPENROUTER_API_KEY").then(|| "env-test-private".to_owned())
    });
    assert_eq!(
        env.get("openrouter").unwrap().unwrap().expose(),
        "env-test-private"
    );
    assert!(env.get("google").unwrap().is_none());
    assert_eq!(env.get("bad/key").unwrap_err(), SecretError::InvalidKey);
    assert_eq!(
        env.set("openrouter", SecretString::new("ignored")),
        Err(SecretError::ReadOnly)
    );
    assert_eq!(env.delete("openrouter"), Err(SecretError::ReadOnly));
}
#[test]
fn fallback_prefers_keychain_and_writes_only_there() {
    let primary = Arc::new(MemorySecrets::default());
    let env = Arc::new(EnvSecrets::with_lookup(|_| Some("fallback-private".into())));
    let fallback = FallbackSecrets::new(primary.clone(), env);
    assert_eq!(
        fallback.get("openrouter").unwrap().unwrap().expose(),
        "fallback-private"
    );
    fallback
        .set("openrouter", SecretString::new("keychain-private"))
        .unwrap();
    assert_eq!(
        fallback.get("openrouter").unwrap().unwrap().expose(),
        "keychain-private"
    );
    fallback.delete("openrouter").unwrap();
    assert!(primary.get("openrouter").unwrap().is_none());
}
#[test]
fn locked_keychain_does_not_silently_fall_back() {
    struct Locked;
    impl SecretStore for Locked {
        fn get(&self, _: &str) -> Result<Option<SecretString>, SecretError> {
            Err(SecretError::Denied)
        }
        fn set(&self, _: &str, _: SecretString) -> Result<(), SecretError> {
            Err(SecretError::Denied)
        }
        fn delete(&self, _: &str) -> Result<(), SecretError> {
            Err(SecretError::Denied)
        }
    }
    let fallback = FallbackSecrets::new(
        Arc::new(Locked),
        Arc::new(EnvSecrets::with_lookup(|_| Some("fallback-private".into()))),
    );
    assert_eq!(fallback.get("openrouter").unwrap_err(), SecretError::Denied);
}
#[test]
#[ignore = "keychain: macOS Keychain, Windows Credential Manager or Linux Secret Service; writes an isolated test service"]
fn native_os_keychain_contract() {
    let store = Keychain::new(format!("vellum-contract-test-{}", std::process::id()));
    suites::model::secrets(&store);
}
