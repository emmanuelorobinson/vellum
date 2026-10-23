---
title: "Cloud"
description: "The optional services behind the Pro features and live collaboration: the account service, the public API, the sync server and the background workers, and how each one stays optional."
---

The cloud is a set of optional services. None of them is needed for local work, and each one can be run by the operator of a deployment, or
left out. The product's local parts are the editor, the daemon and the command line.

## The services

| Service | What it is for |
| --- | --- |
| Account app | Sign-up, billing pages and the share viewer. A Next.js application that uses the shared auth and billing packages |
| Public API | The endpoints the command line, the extension and the viewer call: the account, entitlements, billing and jobs. Hono on Bun, under `/v1/` |
| Sync server | The room service for live collaboration. One room per document. It merges and stores changes, and sends them to connected editors |
| Workers | Background jobs: exports, thumbnails and pull request previews, run by the native command line in a sandboxed folder |
| Viewer | Public pages for share snapshots, served from the account origin |

The services share a Postgres database for accounts and billing, a Redis queue for jobs, and an object store for snapshots and artifacts.
A self-hosted sync server uses a SQLite database and a folder on disk instead, and it needs none of the hosted pieces.

## What stays optional

A service that is not running changes only the feature that depends on it. Editing, exporting, the command line, the MCP server, agents and Git
all work without any of the services. A self-hosted sync server needs no account. Pro features need an account, and the account needs the
services that check it.

## Permissions in the sync server

The sync server checks each role on every change: a viewer reads, a commenter adds comments, and an editor changes the document. The check is
on the server, not in the interface, so a modified client cannot change a document it may only read.

## Share snapshots

A share snapshot is an explicit copy. Creating a link uploads a snapshot of the document and what it needs: its images, fonts, libraries and
script files. Updating the link publishes a new snapshot, and only when you ask. Revoking the link disables public access at once. A copy someone
already downloaded stays with them, so a revocation cannot reach into their files.

## Pull request previews

A pull request preview is a Pro job. A worker takes a complete, trusted copy of the repository, renders the frames that changed, and publishes the
images as artifacts. The worker runs with a fixed binary, a fixed argument list, no shell and no credentials in its environment.

## The self-hosted option

The sync server can run on your own server with no account and no hosted dependency. The [sync and self-hosting](/docs/operations/sync-and-self-hosting)
page explains how to run it. The architecture keeps this path real by building the self-hosted adapters, so the hosted and self-hosted services share
one set of ports.
