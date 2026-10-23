---
title: "Sync and self-hosting"
description: "Build and run the room service that carries live collaboration, on your own server, with no account and no hosted dependency."
---

The sync server is the program that carries live collaboration. It keeps one room for each document, merges the changes people and
agents send, stores them, and sends them to every connected editor. It is a separate program from the daemon, and a local design needs
none of it.

> **Availability.** The room service and its protocol are built and tested. The editors are not yet attached to a room, so you can run
> the server, but the editors cannot yet join a live session. See [collaboration](/docs/using-vellum/collaboration).

## Build and run

```sh
cargo build -p vellum-sync-server
target/debug/vellum-sync-server --help
```

The `--help` output lists the commands. Run the server on loopback, or on a private network, and put a reverse proxy in front of it
that terminates HTTPS and WebSocket Secure connections. Turn off the proxy's access logs for the Authorization header and cookies, so
no credential reaches a log.

Browser origins must match an explicit `--origin` exactly. Native clients send no origin.

## Capabilities

Access is granted with a capability: a random credential of at least thirty-two characters. Each capability binds exactly one room, one
actor, one role and an expiry. The roles are viewer, commenter and editor, and the server checks the role on every change, not only in the
interface.

Store a capability in the operating system's keychain, under the service name the brand defines. The administrative command reads it from
there with `--key NAME`, so the credential never appears on the command line or in a file.

## What the server stores

With self-hosting, the server stores room data in a SQLite database and in a folder on disk. It needs no Redis, no cloud storage and no account
service. Its files hold canonical snapshots of each room and the update log, and no credentials.

## Pointing an editor at it

Native hosts connect through a bundled subprocess, `vellum sync`, given the endpoint, the room and the keychain entry name. Production
connections require a certificate that the machine trusts. Plaintext WebSocket is allowed only on loopback, for development.

## Backups

Back up the database and the snapshot folder together, so that the two copies match each other.
