# Editor ports

`Host` covers atomic file replacement, watches, clipboard MIME bytes, theme notifications, safe
external links and daemon discovery. Discovery returns a relayed capability, never a socket or
credential. `host-vscode` and `host-electron` accept a narrow `HostServices` object supplied by the
trusted extension/preload composition. `host-browser` accepts explicitly granted browser services;
without them, file/clipboard/link access is unavailable. Native APIs, IPC framing, dialogs and
daemon authentication are host work in Prompts 55–58; these adapters do not pretend to provide them.
Adapters own their subscriptions, return copies and make disposal explicit. Service implementations
must perform atomic writes and enforce their own host permissions.

`RelayedDaemonClient` takes a host-owned `DaemonRelay`. It coalesces connects, discards stale connect
completion, disconnects on a failed request and never accepts arbitrary RPC or credentials. Its
initial typed control actions are Stop, Pause/Resume all, Stop agent and Undo turn. Native transport
serialization belongs to the host; this is not a new daemon wire protocol. Feature prompts can add
typed session/presence methods as their consumers arrive.

`NoopSyncClient` stays offline and refuses sending. `NoopTelemetry` stays disabled and retains nothing.
`FakeDaemonClient` is manually driven, with no clocks, model provider, listener socket or network.
The test-runner-independent contract suites live in `@vellum/contracts`; adapter tests pass the
same suites an injected in-memory host/engine/relay boundary. No live native bridge is tested here.
