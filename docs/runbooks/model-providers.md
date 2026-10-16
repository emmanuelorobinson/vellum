# Model providers and secret stores

Prompt 32 / ADR-023. The built-in engine consumes these adapters in Prompt 33; provider settings UI is later.

Composition roots construct a provider with a `ModelConfig`, an `Arc<dyn SecretStore>` and an
`Arc<dyn ModelTransport>` (`vellum_model_http::HttpTransport` in production). Config contains a credential
lookup name, never a credential. The endpoint is the full POST URL. Local endpoints can omit the lookup
name; authenticated endpoints must supply one. Select capabilities from the model's actual configuration;
unknown context size is None. Do not infer vision or reasoning support from a provider's name.

Call `complete(request, cancellation).await`, then await each stream `next()`. Errors may arrive on the
stream while headers or data are being received. A Finished event distinguishes Stop, ToolCalls, Length
and Refused. EOF without a terminal protocol event is Truncated. Drop or cancel the stream to close the
request. A failed request is not retried automatically. Tool arguments arrive as complete JSON objects;
text/reasoning deltas stream incrementally. Preserve ToolCall metadata and Continuation blocks exactly
when constructing follow-up requests. A Continuation containing text replaces that text's history part;
it must not be appended a second time alongside the corresponding display delta.

Chat Completions presets cover OpenRouter, DeepSeek, xAI, Moonshot/Kimi, Together, Fireworks, Z.AI,
OpenCode Zen, Ollama and LM Studio. Custom Chat Completions and Responses URLs are supported.
OpenRouter's default model is `deepseek/deepseek-v4.1-flash` as required by the guide. Presets supply
endpoint addresses; they do not claim model capability, availability, context size or authentication status.

Messages deployment endpoints:

| Deployment | Full endpoint | Credential |
|---|---|---|
| Anthropic | `https://api.anthropic.com/v1/messages` | API key |
| Bedrock | `https://bedrock-runtime.<region>.amazonaws.com/anthropic/v1/messages` | Bedrock key/token |
| Vertex | `https://<location>-aiplatform.googleapis.com/v1/projects/<project>/locations/<location>/publishers/anthropic/models/<model>:streamRawPredict` | host-supplied OAuth bearer token |
| Foundry | `https://<resource>.services.ai.azure.com/anthropic/v1/messages` | Foundry API key |
| Custom | explicit Messages URL | API key, or omit for an explicitly unauthenticated local server |

Gemini uses the AI Studio endpoint
`https://generativelanguage.googleapis.com/v1beta/models/<model>:streamGenerateContent?alt=sse`,
with its key in the `x-goog-api-key` header. Keys never belong in URL query strings.
Hosts acquire/refresh cloud tokens separately and store them through SecretStore. The adapters do not
invoke AWS, gcloud or Azure commands. See the official [Bedrock Messages route](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html),
[Foundry Messages deployment](https://platform.claude.com/docs/en/build-with-claude/claude-in-microsoft-foundry),
[Responses streaming](https://developers.openai.com/api/docs/guides/streaming-responses), and
[Gemini thought signatures](https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures).

`Keychain::new(service)` writes only OS credentials. An explicitly selected `EnvSecrets::for_ci()` reads
`VELLUM_OPENROUTER_API_KEY`, `VELLUM_ANTHROPIC_API_KEY`, etc.; it refuses outside CI. Inject `with_lookup`
in tests instead of mutating process environment. `FallbackSecrets` prefers a present keychain entry;
locked/unavailable keychain errors propagate. Clearing a key cannot clear an environment variable.
Never log a vendor error or format a raw credential; no SecretString serialization is provided.

Validation (both telemetry opt-outs should be set when using pnpm):

- `cargo test --offline -p vellum-contracts --test model`
- `cargo test --offline -p vellum-testkit --test providers --test canary` (no network)
- `node scripts/model-providers.mjs --offline` (tagged model-http; loopback sockets, ephemeral canary,
  full repo/local-state/log scan; release scanner avoids scanning build output in debug mode)
- `node scripts/skia-build.mjs test --offline -p vellum --test write_tools a_scripted_agent -- --ignored`
  (tagged model-http; the scripted provider-to-registry retry path, needs cached/source-built Skia)
- `cargo test --offline -p vellum-secrets-keyring --test secrets`
- `cargo test --offline -p vellum-secrets-keyring --test secrets native_os_keychain_contract -- --ignored`
  (tagged keychain; writes/deletes a process-specific test service in the actual OS keychain; Linux needs
  an unlocked Secret Service session, macOS Keychain, Windows Credential Manager)

The HTTP CI matrix uses no real accounts/keys. Native keychain CI is opt-in on macOS/Windows. Linux
native keychain execution needs a separately provisioned Secret Service session. None of these tests
establishes live-provider compatibility or the later engine's context-management/interrupt budget.
