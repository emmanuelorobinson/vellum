---
title: "Updates and releases"
description: "How {{brand}} is updated today, what a public release needs, and which items are still open before the first one."
---

## Updating today

There is no installer and no automatic update. To update a build from source, pull the new revision of the repository and rebuild.

```sh
git pull
pnpm install --frozen-lockfile
node scripts/skia-build.mjs build -p vellum
```

Then rebuild the extension or the desktop app if you use one, and stop the old daemon with `vellum daemon stop` so that the new build starts
the next time.

## What a public release needs

The release pipeline is built. It checks that the evidence for a release exists, at one commit, from recent passing runs. It does not
establish that the product is ready. The [release runbook](/docs/operations/runbooks/release) lists the blockers the release policy records,
and the evidence each one needs. The main groups are these:

- **Signing.** A Developer ID identity and notarisation on macOS, and an Authenticode certificate on Windows.
- **Update delivery.** An HTTPS update feed with a signing key that the application checks before it installs anything.
- **Publishing accounts.** The npm package scope, the Marketplace and Open VSX publisher namespaces, and the Homebrew tap.
- **Acceptance.** Installed-host tests on each platform, the visual and contrast gates, and the parity inventory's remaining rows.

None of these has been completed. Nothing has been signed, uploaded or published. The owner of the project provides the accounts and
the signing identities, and the pipeline does not create them.

## What to watch

The [availability](/docs/getting-started/availability) page is the summary. When a release is published, its notes will say which
features changed, and the runbooks will record the evidence for them.
