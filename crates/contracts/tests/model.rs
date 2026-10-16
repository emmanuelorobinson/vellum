//! TEST DATA: deterministic providers and credentials, never real accounts.
use vellum_contracts::{
    fakes::{MemorySecrets, ScriptedModel},
    suites,
};
use vellum_core::ports::*;

#[test]
fn memory_secrets_obey_the_contract() {
    suites::model::secrets(&MemorySecrets::default());
}

#[test]
fn scripted_provider_obeys_the_contract() {
    let provider = ScriptedModel::new(vec![vec![
        Ok(ModelEvent::Text("ready".into())),
        Ok(ModelEvent::Finished(FinishReason::Stop)),
    ]]);
    suites::model::provider(&provider);
}

#[test]
fn secrets_never_format_as_plaintext() {
    let secret = SecretString::new("contract-only-private-value");
    assert_eq!(format!("{secret:?}"), "SecretString([redacted])");
}

#[test]
fn fake_reports_exhaustion_and_cancellation() {
    let provider = ScriptedModel::new(vec![vec![]]);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(matches!(
        suites::model::block_on(provider.complete(ChatRequest::text("test", "hi"), cancel)),
        Err(ModelError::Cancelled)
    ));
    assert!(
        suites::model::block_on(
            provider.complete(ChatRequest::text("test", "hi"), Cancellation::default())
        )
        .is_ok()
    );
    assert!(matches!(
        suites::model::block_on(
            provider.complete(ChatRequest::text("test", "hi"), Cancellation::default())
        ),
        Err(ModelError::Exhausted)
    ));
}

#[test]
fn replay_transport_obeys_the_wire_contract() {
    let transport = vellum_contracts::fakes::ReplayTransport::new(vec![vec![
        Ok(WireEvent::Data(serde_json::json!({"probe":"ready"}))),
        Ok(WireEvent::Done),
    ]]);
    suites::model::transport(&transport, "https://example.invalid/provider");
}

#[test]
#[should_panic(expected = "ModelProvider: missing terminal event")]
fn the_contract_rejects_a_provider_that_ends_without_a_terminal_event() {
    suites::model::provider(&ScriptedModel::new(vec![vec![Ok(ModelEvent::Text(
        "partial".into(),
    ))]]));
}
