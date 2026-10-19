// TEST DATA: pure state and URL validation, no network or credentials.
use vellum_account_http::{AccountUrl, PollState, PollStatus};
#[test]
fn credentials_require_a_clean_secure_origin() {
    for origin in [
        "https://account.example.test",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
    ] {
        assert!(AccountUrl::parse(origin).is_ok());
    }
    for origin in [
        "http://account.example.test",
        "https://person:password@example.test",
        "https://example.test/?token=x",
        "https://example.test/path",
        "file:///tmp/account",
    ] {
        assert!(AccountUrl::parse(origin).is_err());
    }
}
#[test]
fn polling_obeys_interval_slowdown_expiry_and_terminal_errors() {
    let mut state = PollState::new(1000, 30, 5).unwrap();
    assert_eq!(state.wait_ms(1000).unwrap(), 5000);
    state.update(PollStatus::Pending, 6000).unwrap();
    assert_eq!(state.wait_ms(6000).unwrap(), 5000);
    state.update(PollStatus::SlowDown, 11000).unwrap();
    assert_eq!(state.wait_ms(11000).unwrap(), 10000);
    assert!(state.wait_ms(31000).is_err());
    assert!(state.update(PollStatus::Denied, 21000).is_err());
    assert!(PollState::new(0, 0, 5).is_err());
}
#[test]
fn stored_credentials_are_opaque_and_roundtrip_only_in_the_secret_store() {
    use vellum_account_http::StoredSession;
    use vellum_core::ports::SecretString;
    let stored = StoredSession::new(
        AccountUrl::parse("https://account.example.test").unwrap(),
        SecretString::new("test-only-session-token"),
    )
    .unwrap();
    let encoded = stored.encode().unwrap();
    let restored = StoredSession::decode(encoded).unwrap();
    assert_eq!(restored.origin.as_str(), "https://account.example.test/");
    assert_eq!(restored.token.expose(), "test-only-session-token");
    assert!(
        StoredSession::new(
            AccountUrl::parse("https://account.example.test").unwrap(),
            SecretString::new("bad\nheader")
        )
        .is_err()
    );
}
