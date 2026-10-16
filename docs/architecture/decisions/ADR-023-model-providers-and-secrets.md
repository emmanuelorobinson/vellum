# ADR-023: Model providers and secrets

Status: proposed; implemented in Prompt 32. Only a person synchronizes ARCHITECTURE.md.

## Decision

Core owns runtime-independent, object-safe ModelProvider/ModelStream, ModelTransport and SecretStore
ports. Futures use std; no vendor/network/clock/filesystem dependency enters the pure crates.
Conversation parts include native tool calls/results, inline images and opaque continuation blocks.
Usage values are optional, not fabricated zeroes. Selected-model capabilities are explicit;
unsupported features fail before transport. Errors are payload-free enums, never server bodies,
URLs or vendor error chains. SecretString is not serializable and formats as redacted.

Four independent adapters handle Chat Completions, Responses, Messages and Gemini. Composition roots
inject the model-http adapter, so adapters never import each other. Complete POST URLs are explicit,
including custom paths. Chat Completions has ten presets and the guide's exact OpenRouter default
model; no catalog fetch or request occurs on construction. Responses sets store:false. Hosts must
preserve opaque reasoning/signature blocks on follow-up requests; display deltas do not replace them.

Messages supports Anthropic/custom Messages, Foundry api-key, Vertex bearer credentials with the
vertex-2023-10-16 body version, and Bedrock's native /anthropic/v1/messages SSE route with x-api-key.
The Bedrock route is verified against current AWS documentation, rather than the reference's legacy
InvokeModel binary event stream. Hosts supply/refresh cloud credentials; no ambient SDK/command
credential discovery or SigV4 implementation is added. Credentials present means API key set;
only a terminal protocol completion establishes Connected. SignedIn remains reserved for actual
account sign-in established by a later host.

The HTTP adapter owns one lazy Tokio runtime. Requests race cancellation and receiver closure,
including while waiting for headers/body or channel capacity. Dropping/erroring/completing a decoded
stream drops its wire; cancellation also clears queued tool calls. Redirects are refused. HTTPS is
required except loopback HTTP; userinfo, fragments and arbitrary query keys are refused (alt and
api-version are allowed). SSE parsing handles split UTF-8/CRLF/JSON and multiline events. Limits:
1 MiB/event, 16 MiB/stream, 32 queued wire events, 128 concurrent tool calls. Truncated streams fail.
There are no automatic retries; the engine decides whether replay is safe.
Credential echoes are redacted at the transport and decoded-event boundaries. The latter holds only
a suffix that could begin the credential, covering keys split over text deltas; tool JSON keys/values
and opaque blocks are scrubbed too. An error discards held suffixes. This does not sanitize arbitrary
unrelated secrets supplied by a user in a prompt.

SecretStore::get returns Result to distinguish a missing entry from a locked/unavailable keychain;
delete supports Clear key. Keyring 4.2's v1 interface selects real native stores. Environment lookup is
explicit, namespaced VELLUM_<PROVIDER>_API_KEY, read-only, and CI-gated in its ambient constructor.
Fallback occurs only for missing entries, never keychain errors; CI can select EnvSecrets directly.
Writes/deletes never persist to files or the environment.

## Dependencies and policy

reqwest =0.13.5 (rustls/json, no defaults), Tokio =1.53.2, async-channel =2.5.0 and url =2.5.8 stay
in model-http. Rustls resolves to patched 0.23.46 after cargo-deny reported advisories in 0.23.45.
No advisory ignore was added. Keyring =4.2.0 stays inside secrets-keyring. The wrapper policy lists
Tokio's immediate HTTP parents and SHA-256's Secret Service parent; reqwest and secret-service are
also wrapper-banned. The pure-crate dependency allowlist is unchanged.
Only webpki-root-certs =1.0.9 gets a CDLA-Permissive-2.0 root-data licence exception after reading the
agreement. Its agreement is included in the adapter's notices for redistribution. Other packages
under that licence are not allowed by this exception.

## Reference study (no source ported)

Goose's named provider definitions separate deployment/authentication from protocol handling; its
cloud adapters additionally own retries and credential discovery. Vellum keeps the separation but
requires supplied credentials and leaves retries to the engine to avoid duplicate tool work.
OpenCode's provider facade separates deployment from model selection; Vellum keeps explicit model
capabilities. Codex's keyring store distinguishes missing entries and avoids payload-bearing error
chains; Vellum uses enum-only errors. The sibling outpost secret type demonstrates redacted formatting;
Vellum also omits serialization. Read the guide's named paths/READMEs at their existing reference pins.

## Evidence and limits

Shared contracts run on memory fakes and all four adapters. The loopback stub consumes ordered
transcripts, intentionally splits bytes, keeps no credential header values and fails on exhaustion.
Prompt 30's pending scripted-agent test now drives the model through registry calls, receives a
bad-width error, retries and builds a frame through Guard/core. This test driver is not the built-in
engine (Prompt 33).
The canary skeleton generates a fresh credential, captures provider diagnostics/test logs and scans
all ordinary files under the repo, ~/.vellum when present and its temporary artifacts. Symlinks are
not followed; only matching paths are reported. A binary cross-buffer regression tests the scanner.
Full engine/session/permissions/export/share/UI-frame coverage is added when those paths exist.
No paid provider, cloud IAM setup, installed engine or real MCP client was exercised.
