# TEST DATA: installed-engine protocol fixtures

`claude-code/2.1.301/success.recorded.ndjson` was recorded from installed Claude Code 2.1.301 with the
fixed read-only recorder prompt. Its adjacent JSON manifest distinguishes it from synthetic data.
It retains only decoder fields; auth/init identity, native diagnostic text and raw stderr are excluded.
The companion `.synthetic.ndjson` covers deterministic protocol combinations and is not a live recording.
Negative authentication/usage-limit protocol cases live inline in the replay tests; no account was logged
out or exhausted to generate them. `fake-claude.py` is the native process-boundary fixture, not Claude Code.

Re-record with `bash scripts/record-engine-fixtures.sh`, then review the projected output and update
version-specific replay expectations when usage changes. No raw provider transcript or credential belongs
here. See `docs/runbooks/installed-engines.md` for the account-free and real engines lanes.


`codex/0.158.0/success.recorded.ndjson` is the real app-server projection: correlated handshake, a direct
get_app_state request, assistant text and inclusive cumulative usage. `exec-success.recorded.ndjson` is the
real exec fallback projection; tool execution arrives separately through the private MCP relay. Both
manifests record transport and successful canvas calls. Synthetic fixtures and inline negative cases are
marked TEST DATA. `fake-codex.py` is the account-free native subprocess fixture. No native auth/config,
identity, stderr or diagnostic text is saved. Re-record with `--engine codex` (and `--transport exec` for
fallback) and review both projections before updating replay expectations.
