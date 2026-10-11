#[test]
fn crate_identifies_itself() {
    assert_eq!(
        vellum_llm_openai_compat::CRATE_NAME,
        "vellum-llm-openai-compat"
    );
}
