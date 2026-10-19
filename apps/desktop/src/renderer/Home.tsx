import { brand, copy } from "@vellum/config";
import type {
  DesktopBridge,
  DesktopRequest,
  DesktopState,
  HomeFile,
} from "@vellum/host-electron/bridge";
import { useEffect, useRef, useState } from "react";

export function Home({ state, bridge }: { state: DesktopState; bridge: DesktopBridge }) {
  const [tab, setTab] = useState("Recents");
  const [layout, setLayout] = useState("Grid");
  const [sort, setSort] = useState("Last opened");
  const [selected, setSelected] = useState<string[]>([]);
  const [problem, setProblem] = useState(false);
  const [busy, setBusy] = useState(false);
  const recoveryDialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (state.recovery && recoveryDialog.current && !recoveryDialog.current.open)
      recoveryDialog.current.showModal();
  }, [state.recovery?.id]);
  const run = async (request: DesktopRequest) => {
    if (busy) return;
    setBusy(true);
    setProblem(false);
    try {
      const result = await bridge.request(request);
      if (!result.ok) setProblem(true);
      else if (request.type === "forget") setSelected([]);
    } catch {
      setProblem(true);
    } finally {
      setBusy(false);
    }
  };
  const button = (label: string, request: DesktopRequest, primary = false) => (
    <button
      className={primary ? "primary" : undefined}
      type="button"
      disabled={busy}
      onClick={() => void run(request)}
    >
      {label}
    </button>
  );
  const page = state.page ?? "home";
  const updates = state.updates;
  const account = state.account ?? { kind: "idle" };
  const files = (state.home?.files ?? [])
    .filter((f) => tab === "Recents" || (tab === "Drafts" && f.kind === "draft"))
    .sort((a, b) =>
      sort === "Name"
        ? a.name.localeCompare(b.name)
        : sort === "Created"
          ? b.created - a.created
          : b.opened - a.opened,
    );
  const date = (n: number) =>
    new Date(n).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  const file = (f: HomeFile) => (
    <article className="home-file" data-selected={selected.includes(f.id)} key={f.id}>
      <button
        type="button"
        className="file-open"
        onClick={(event) => {
          if (event.metaKey || event.ctrlKey)
            setSelected((ids) =>
              ids.includes(f.id) ? ids.filter((id) => id !== f.id) : [...ids, f.id],
            );
          else void run({ type: "homeOpen", id: f.id });
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          setSelected([f.id]);
        }}
      >
        {layout === "Grid" && f.kind !== "draft" && (
          <span className="file-thumbnail">Preview unavailable</span>
        )}
        <strong>{f.name}</strong>
        <span className="mono">{f.folder}</span>
        {f.kind === "draft" ? (
          <span>Auto-saved · on your computer · {date(f.opened)}</span>
        ) : (
          <>
            <span className="mono">{f.branch ?? "Branch unavailable"}</span>
            <span>
              {date(f.opened)}
              {f.running === true
                ? " · ▣ Working"
                : f.running === null
                  ? " · Session state unavailable"
                  : ""}
            </span>
          </>
        )}
      </button>
      {f.kind === "draft" && (
        <div className="home-actions">
          {button("Save As…", { type: "draftSaveAs", id: f.id }, true)}
          {button("Delete draft", { type: "draftDelete", id: f.id })}
        </div>
      )}
    </article>
  );
  const composer = (
    <section className="home-composer" aria-label="Design prompt">
      <h2>{copy.homeComposer}</h2>
      <textarea
        aria-label={copy.homeComposer}
        placeholder="Describe a screen, or attach DESIGN.md…"
        disabled
      />
      <div className="home-actions spread">
        {button(state.folderName || "Choose a folder…", { type: "openFolder" })}
        <button
          className="primary"
          type="button"
          disabled
          title="The document engine and agent request service are unavailable in this build."
        >
          Start designing
        </button>
      </div>
      <span className="unavailable">⊘ Sending a design prompt is unavailable in this build.</span>
    </section>
  );
  return (
    <main
      className="desktop-home"
      data-platform={state.platform}
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => e.preventDefault()}
    >
      <header className="home-appbar">
        <span>{brand.name}</span>
        <nav aria-label="Desktop">
          {button("Home", { type: "home" })}
          {button("Agents", { type: "agents" })}
          {button("Sign in", { type: "signin" })}
          {state.active && button("Back to editor", { type: "editor" })}
        </nav>
      </header>
      {page === "agents" ? (
        <section className="home-content agents-page">
          <h1>Agents and daemon</h1>
          <div className="home-actions daemon-state">
            <span role="status">{state.daemon ? "✓ Running" : `⊘ ${copy.daemonOffline}`}</span>
            {button("Reconnect", { type: "restartDaemon" })}
            {button("Pause all agents", { type: "pauseAll" })}
          </div>
          <h2>Built-in engines</h2>
          <p>⊘ Provider keys and engine controls are unavailable in this build.</p>
          <p className="mono">Usage this week · unavailable</p>
          <h2>Installed agents and MCP clients</h2>
          {(state.agentSettings?.clients ?? []).map((c) => (
            <div className="agent-config" key={c.id}>
              <div>
                <strong>{c.name}</strong>
                <div className="mono">{c.path}</div>
                <p>
                  {c.state === "connected"
                    ? "✓ Connected"
                    : c.state === "configured"
                      ? "◇ Configured"
                      : c.state === "ready"
                        ? "◇ Ready to connect"
                        : c.state === "error"
                          ? "⊘ Configuration unavailable"
                          : "⊘ Not detected"}{" "}
                  · Last activity unavailable
                </p>
              </div>
              {button(
                c.state === "configured" || c.state === "connected" ? "Reconnect" : "Connect",
                { type: "agentConfig", id: c.id, action: "connect" },
              )}
              {(c.state === "configured" || c.state === "connected") &&
                button("Remove", { type: "agentConfig", id: c.id, action: "disconnect" })}
              <details>
                <summary>Manual setup</summary>
                <pre>{c.entry}</pre>
                {button("Copy", { type: "clipboardWrite", text: c.entry })}
              </details>
            </div>
          ))}
          {!state.agentSettings && <p>⊘ Agent configuration unavailable</p>}
          <h2>Headless mode</h2>
          <p>
            Agents can read and edit design files without an editor window. Pausing agents rejects
            further writes until you resume.
          </p>
          <label>
            <input
              type="checkbox"
              checked={state.notifications ?? false}
              onChange={(e) => void run({ type: "notifications", enabled: e.target.checked })}
            />
            Notify when a background turn ends, fails, or needs permission
          </label>
          <p>Notifications follow recorded agent turns in open files.</p>
        </section>
      ) : page === "signin" ? (
        <section className="signin-page" data-device={account.kind === "waiting"}>
          <div>
            <h1>
              {account.kind === "waiting" ? "Approve in your browser" : `Sign in to ${brand.name}`}
            </h1>
            {account.kind !== "waiting" && (
              <>
                <p>Enable share links and cloud sync for files you choose.</p>
                <p>{copy.syncScopeA05}</p>
              </>
            )}
            {account.kind === "waiting" ? (
              <>
                <p>Compare this code with the approval page before connecting this app.</p>
                <p className="mono device-code">{account.userCode}</p>
                <p className="mono device-url">{account.verificationUrl.split("?")[0]}</p>
                <div className="home-actions">
                  {button("Open approval page", { type: "accountOpen" }, true)}
                  {button("Cancel sign-in", { type: "accountCancel" })}
                </div>
                <p role="status">◇ Waiting for approval</p>
              </>
            ) : account.kind === "busy" ? (
              <>
                <p role="status">◇ Contacting your account service…</p>
                {button("Cancel", { type: "accountCancel" })}
              </>
            ) : (
              <>
                {account.kind === "signed-in" ? (
                  <p role="status">✓ Signed in</p>
                ) : (
                  <>
                    {button("Continue in browser", { type: "accountLogin" }, true)}
                    {account.kind === "failed" && (
                      <p role="alert">
                        ⊘ Account request unavailable. Try again when your account service is
                        available.
                      </p>
                    )}
                    {account.kind === "signed-out" && (
                      <p role="status">
                        ✓ Signed out
                        {account.remoteRevoked === false
                          ? " on this computer; remote session revocation unconfirmed"
                          : ""}
                      </p>
                    )}
                  </>
                )}
                <div className="home-actions">
                  {button("Check account", { type: "accountRefresh" })}
                  {button("Sign out", { type: "accountLogout" })}
                </div>
              </>
            )}
            <small>Local editing and agents work without an account.</small>
          </div>
        </section>
      ) : (
        <section className={`home-content ${state.welcome ? "welcome-page" : ""}`}>
          {state.welcome ? (
            <section className="welcome-content">
              <div className="wordmark">{brand.name.toLowerCase()}</div>
              <h1>{brand.tagline}</h1>
              <p>
                Open a folder to design next to your code.
                <br />
                Your files stay on your computer.
              </p>
              <div className="home-actions">
                {button("Open a folder", { type: "openFolder" }, true)}
                {button("New file", { type: "newDraft" })}
                {button("Connect your coding agent", { type: "agents" })}
              </div>
            </section>
          ) : (
            <>
              <div className="home-actions spread">
                <h1>{state.folderName || "Home"}</h1>
                <div className="home-actions">
                  {button("New file", { type: "newDraft" }, true)}
                  {button(state.folderName ? "Open files…" : "Open a folder", {
                    type: state.folderName ? "openFiles" : "openFolder",
                  })}
                </div>
              </div>
              {composer}
              <div className="home-actions spread">
                <div role="tablist" aria-label="Home files">
                  {["Recents", "Drafts", "Design systems"].map((t) => (
                    <button
                      role="tab"
                      aria-selected={t === tab}
                      type="button"
                      key={t}
                      onClick={() => {
                        setTab(t);
                        setSelected([]);
                      }}
                    >
                      {t}
                    </button>
                  ))}
                </div>
                <div className="home-actions">
                  <label>
                    View{" "}
                    <select value={layout} onChange={(e) => setLayout(e.target.value)}>
                      <option>Grid</option>
                      <option>List</option>
                    </select>
                  </label>
                  <label>
                    Sort{" "}
                    <select value={sort} onChange={(e) => setSort(e.target.value)}>
                      <option>Last opened</option>
                      <option>Name</option>
                      <option>Created</option>
                    </select>
                  </label>
                </div>
              </div>
              {selected.length > 0 && (
                <div className="home-actions">
                  <span>{selected.length} selected</span>
                  {button("Open selected", { type: "openSelected", ids: selected })}
                  {files.filter((f) => selected.includes(f.id)).every((f) => f.kind === "file") &&
                    button("Remove from recents", { type: "forget", ids: selected })}
                  <button type="button" onClick={() => setSelected([])}>
                    Clear selection
                  </button>
                </div>
              )}
              {tab === "Design systems" ? (
                <p role="status">⊘ Bundled design systems are unavailable in this build.</p>
              ) : files.length ? (
                <div className={`home-files ${layout.toLowerCase()}`}>{files.map(file)}</div>
              ) : (
                <p role="status">{copy.homeEmpty}</p>
              )}
              <h2>Recent folders</h2>
              {state.home?.folders.map((f) => (
                <button
                  className="home-folder mono"
                  key={f.id}
                  type="button"
                  onClick={() => void run({ type: "folderRecent", id: f.id })}
                >
                  {f.label}
                </button>
              ))}
              <small>
                {state.platform === "mac" ? "⌘" : "Ctrl"}-click to select several · Right-click to
                remove from recents · Drop a .vellum file to open it
              </small>
            </>
          )}
          {state.welcome && composer}
        </section>
      )}
      <footer className="home-footer">
        {button("Check for Updates", { type: "checkUpdates" })}
        <span role="status">
          {updates?.kind === "unavailable"
            ? "Updates require a signed release and update feed."
            : updates?.kind === "failed"
              ? "⊘ Update verification or download failed. Your installed version is kept."
              : updates?.kind === "current"
                ? "✓ Up to date"
                : updates?.kind === "available" || updates?.kind === "ready"
                  ? `${updates.version} · ${updates.kind}`
                  : updates?.kind === "checking"
                    ? "Checking for updates…"
                    : updates?.kind === "downloading"
                      ? "Downloading and verifying…"
                      : ""}
        </span>
        {updates?.kind === "available" && button("Download update", { type: "downloadUpdate" })}
        {updates?.kind === "ready" && button("Restart and install", { type: "installUpdate" })}
      </footer>
      {problem && (
        <div role="alert" className="home-error">
          The action could not be completed. Check the file and daemon connection, then try again.
        </div>
      )}
      {state.recovery && (
        <dialog
          ref={recoveryDialog}
          onCancel={(event) => {
            event.preventDefault();
            if (state.recovery)
              void run({ type: "recovery", id: state.recovery.id, action: "cancel" });
          }}
          aria-label="Recovered changes"
          className="recovery-dialog"
        >
          <h2>Recovered changes</h2>
          <p>
            {brand.name} found unsaved changes to {state.recovery.name} from{" "}
            {date(state.recovery.savedAt)}.
          </p>
          <div className="recovery-preview">⊘ Before/after previews unavailable</div>
          <p>Recovery copies are temporary backups. Full history is in Git.</p>
          <div className="home-actions">
            {button("Save", { type: "recovery", id: state.recovery.id, action: "save" }, true)}
            {button("Save As…", { type: "recovery", id: state.recovery.id, action: "saveAs" })}
            {button("Discard backup", {
              type: "recovery",
              id: state.recovery.id,
              action: "discard",
            })}
            {button("Cancel", { type: "recovery", id: state.recovery.id, action: "cancel" })}
          </div>
        </dialog>
      )}
    </main>
  );
}
