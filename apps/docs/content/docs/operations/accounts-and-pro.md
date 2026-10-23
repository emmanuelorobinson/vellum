---
title: "Accounts and Pro"
description: "Which features need an account, how device sign-in works, what a Pro entitlement grants, and what an operator must provide before the account service exists."
---

Local work never needs an account. Editing, files, exports, the command line, the MCP server, the agents and Git all run on your
computer and never contact the account service. An account is needed only for the Pro features: share links, pull request previews,
cloud sync and premium design systems.

> **Availability.** The account service is built, and its tests run against stand-ins. No public account service exists, so Pro
> features are not available to anyone yet. The [availability](/docs/getting-started/availability) page has the current status.

## Signing in

Sign-in uses a device code, so no password is typed into the command line.

```sh
vellum login --account-url https://your-account-service.example
vellum whoami
vellum logout
```

The first sign-in needs the address of the account service, given with `--account-url` or in `VELLUM_ACCOUNT_URL`. Later sign-ins
reuse the address saved with the session in your keychain. `login` prints a URL. You open it in a browser, sign in by email, check that
the code shown in the browser matches the one in your terminal, and approve. The approval expires after ten minutes.

`logout` removes the local sign-in, even when you are offline. It reports whether the server confirmed the revocation.

## Pro capabilities

The billing package names four Pro capabilities: cloud sync, share links, pull request previews and premium design systems. Each one is
checked against the account's current subscription every time it is used. A subscription that has lapsed grants nothing, and a
cancellation keeps the paid time that remains. There is no offline copy of a Pro entitlement, so a lapsed subscription stops working as
soon as the service confirms it.

## What the service stores

The service stores the account, the device sessions, the subscription state and the jobs it runs for a Pro account. Sessions last at
most thirty days, and each can be listed and revoked. The service does not store design files for local work. A share link stores a
snapshot you chose to publish.

## For the person who runs the service

An operator provides the database, the cache, the mail sender, the payment provider's settings and the worker's storage. Every value is
supplied by the deployment's secret store. None is stored in the repository. The
[accounts runbook](/docs/operations/runbooks/accounts) names each setting and the checks that exist, and the
[sharing runbook](/docs/operations/runbooks/sharing) names the storage a share needs.
