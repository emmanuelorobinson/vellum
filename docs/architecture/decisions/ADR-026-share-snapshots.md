# ADR-026: Explicit portable share snapshots

Status: Accepted for implementation, pending live storage/platform and visual acceptance.

Prompt 62. Sharing is an explicit copy. It never changes a DocStore or enables live sync.

The native daemon collects the current document through its DocumentTarget and only typed dependencies named by
that document and its transitive libraries. It canonicalizes each document to a single `.vellum` file while
retaining repo-relative paths. Referenced images/fonts are hash checked. Missing paths require an explicit
Share anyway action. Native snapshot reads refuse symlinks, hidden paths, special files and root escapes.
The transport envelope contains inert base64 file bytes, missing paths, and derived viewer resources. The
`.vellum` files remain authoritative; the envelope is not a document format. Scripts run only in the existing
bounded native sandbox to freeze their derived output. They never execute in the viewer.

A daemon-owned thread renders the frozen inputs, then uploads immutable 512 KiB chunks. Its private spool
holds the immutable files and public upload identifiers, never credentials. Only the worker reads the account
session from the OS keychain. Disconnecting the requesting editor does not cancel the job. A failed/restarted
job resumes with Retry. An upload expires after 24 hours; completed chunks are immutable. Four uploads may run
at once. The envelope is limited to 64 MiB, 2,048 files, 256 libraries, 100 visible roots and 32 scripts.

The ObjectStore port has atomic create-if-absent and compare-and-swap writes. R2 implements these via its S3
conditional request headers; the memory adapter has the same contract. Application chunks are separate R2
objects, not S3 multipart parts. A private share record atomically switches to a fully checksummed snapshot.
The prior snapshot remains public until that switch. Permanent revocation tombstones win over in-flight
publication. Owner operations authenticate every request; upload/publication recheck `share_links`. Owners
can revoke and recipients can read/download after the owner's subscription expires.

Public links use 256-bit random identifiers. Every read/download checks the current private record; there
are no public buckets or long-lived signed object URLs. Responses disable caches and referrers. Deletion
revokes access; already loaded/downloaded copies cannot be recalled. Orphan chunk retention and private
object/spool cleanup are operator concerns documented in the runbook.

The viewer uses `host-browser` with exact bundle-member reads and rejected writes, and the existing
`vellum-wasm`/Skia display-list ABI. The native core prepares layout, spec and CSS from the same frozen
inputs. This does not depend on or invent the missing editable document ABI from Prompt 55. ZIP method 0
preserves exact file bytes; paths are validated for portable extraction. Open in Vellum uses the registered
native protocol. After a native destination picker, the app downloads
the public envelope without credentials and installs exact files in a new exclusive directory. Both ZIPs
and native imports add an empty host root marker so sibling library paths resolve without creating Git
metadata. Uploaded bundles cannot supply hidden marker files. Native protocol registration happens only
in packaged builds; development acceptance injects the OS event and does not change system defaults.

The account origin proxies the fixed API surface, including binary chunks, without redirects. A standalone
web origin proxies public share reads only. W03 uses existing sessions and Polar's customer portal for plan,
billing and invoices. It does not infer device names that the account session schema does not provide.

Evidence: storage/proxy/native collector and script-output regressions; actual Rust snapshot → API → WASM
pixels → exact ZIP bytes → revocation acceptance. No source was ported. No §3.2 reference row applies to P62.
