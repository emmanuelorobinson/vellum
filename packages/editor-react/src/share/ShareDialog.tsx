"use client";

import type { CollaborationController } from "@vellum/editor-core";
import type { SharePort, ShareRequest, ShareState } from "@vellum/host-api";
import { useEffect, useRef, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { LiveShareSection } from "../collaboration/Collaboration";
import { Icon } from "../icons/Icon";
import "./share.css";
export function ShareDialogBody({
  name,
  state: s,
  act,
  close,
  openViewer,
  busy = false,
  canManage = true,
}: {
  name: string;
  state: ShareState;
  act: (request: ShareRequest) => void;
  close: () => void;
  openViewer?: () => void;
  busy?: boolean;
  canManage?: boolean;
}) {
  const { t: translateUi } = useLocale();
  const time = s.snapshot
    ? new Intl.DateTimeFormat("en-GB", {
        day: "numeric",
        month: "short",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
        timeZone: "UTC",
      }).format(s.snapshot.at)
    : "";
  const status = {
    "not-shared": "○ Not shared",
    checking: "△ Checking · missing files",
    uploading: `◌ Uploading · ${Math.round((s.progress ?? 0) * 100)}%`,
    shared: "✓ Shared · View only",
    changed: "○ Changed since shared",
    failed: "⚠ Upload failed",
  }[s.state];
  return (
    <>
      <h2 id="share-title">
        {translateUi("Share")} {name}
      </h2>
      <p role="status" data-state={s.state}>
        {status}
      </p>
      {s.state === "not-shared" && (
        <p>
          Upload this file, its images, fonts, scripts and imported libraries. Your other repo files
          stay on your computer.
        </p>
      )}
      {s.state === "checking" && (
        <>
          <p>These files are missing. Sharing anyway marks the snapshot incomplete.</p>
          <ul>
            {s.missing?.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </>
      )}
      {s.state === "uploading" && (
        <>
          <p>Uploading continues if you close this dialog.</p>
          <progress value={s.progress ?? 0} max={1} aria-label="Upload progress" />
        </>
      )}
      {s.state === "shared" && (
        <>
          <a className="vs-url" href={s.url} target="_blank" rel="noreferrer">
            {s.url}
          </a>
          <p>
            Snapshot from {time} UTC{s.snapshot?.incomplete ? " · △ Incomplete snapshot" : ""}
          </p>
        </>
      )}
      {s.state === "changed" && (
        <p>
          New changes since{" "}
          {s.snapshot
            ? new Intl.DateTimeFormat("en-GB", {
                hour: "2-digit",
                minute: "2-digit",
                hour12: false,
                timeZone: "UTC",
              }).format(s.snapshot.at)
            : "the last snapshot"}
          . The link still shows the earlier snapshot.
        </p>
      )}
      {s.state === "failed" && (
        <p>{s.message ?? "The connection was interrupted. Retry to resume the upload."}</p>
      )}
      <div className="vs-actions">
        {s.state === "not-shared" && (
          <button
            disabled={busy || !canManage}
            type="button"
            data-primary=""
            onClick={() => act({ action: "create" })}
          >
            Create share link
          </button>
        )}
        {s.state === "checking" && (
          <>
            <button
              disabled={busy || !canManage}
              type="button"
              data-primary=""
              onClick={() => act({ action: "check" })}
            >
              Restore and try again
            </button>
            <button
              disabled={busy || !canManage}
              type="button"
              onClick={() =>
                act({ action: "create", shareAnyway: true, ...(s.id ? { id: s.id } : {}) })
              }
            >
              Share anyway
            </button>
          </>
        )}
        {s.state === "failed" && (
          <button
            disabled={busy || !canManage}
            type="button"
            data-primary=""
            onClick={() => act({ action: "retry" })}
          >
            {translateUi("Retry")}
          </button>
        )}
        {s.state === "changed" && (
          <button
            disabled={busy || !canManage}
            type="button"
            data-primary=""
            onClick={() => act({ action: "create", id: s.id })}
          >
            Update snapshot
          </button>
        )}
        {s.state === "shared" && (
          <button
            disabled={busy}
            type="button"
            data-primary=""
            onClick={() => act({ action: "status" })}
          >
            Copy link
          </button>
        )}
        {(s.state === "shared" || s.state === "changed") && (
          <button disabled={busy} type="button" onClick={openViewer}>
            Open viewer
          </button>
        )}
        {s.state === "shared" && (
          <button
            disabled={busy || !canManage}
            type="button"
            onClick={() => act({ action: "delete", id: s.id })}
          >
            Delete share link
          </button>
        )}
        {s.state !== "shared" && s.state !== "changed" && (
          <button disabled={busy} type="button" onClick={close}>
            {s.state === "uploading" ? "Close" : "Cancel"}
          </button>
        )}
      </div>
      {s.state === "shared" && (
        <>
          <p className="vs-small">
            A snapshot of this file, its images, fonts, scripts and libraries. Update it explicitly
            after making changes.
          </p>
          <p className="vs-small">Deleting the link does not remove copies already downloaded.</p>
        </>
      )}
    </>
  );
}
export function ShareDialog({
  name,
  port,
  onClose,
  collaboration,
}: {
  name: string;
  port: SharePort;
  onClose: () => void;
  collaboration?: CollaborationController;
}) {
  const { t: translateUi } = useLocale();
  const [state, setState] = useState<ShareState>({ state: "not-shared" });
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(true);
  const [notice, setNotice] = useState("");
  const pending = useRef(false);
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    let live = true;
    const load = async () => {
      try {
        const status = await port.request({ action: "status" });
        if (live) {
          setState(status);
          if (status.state === "not-shared" || status.state === "shared") {
            const checked = await port.request({ action: "check" });
            if (live) setState(checked);
          }
        }
      } catch {
        if (live) setState({ state: "failed" });
      } finally {
        if (live) setBusy(false);
      }
    };
    void load();
    return () => {
      live = false;
    };
  }, [port]);
  useEffect(() => {
    if (state.state !== "uploading") return;
    let live = true;
    const timer = setInterval(() => {
      void port
        .request({ action: "status" })
        .then((s) => {
          if (live) setState(s);
        })
        .catch(() => {
          if (live) setState({ state: "failed" });
        });
    }, 1000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, [port, state.state]);
  const act = async (r: ShareRequest) => {
    if (r.action === "status" && state.url) {
      try {
        await port.copy(state.url);
        setNotice("Link copied");
      } catch {
        setNotice("Copy unavailable");
      }
      return;
    }
    if (r.action === "delete" && !confirm) {
      setConfirm(true);
      return;
    }
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setNotice("");
    try {
      setConfirm(false);
      setState(await port.request(r));
    } catch {
      setState({ ...state, state: "failed" });
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };
  return (
    <dialog
      className={collaboration ? "vs-dialog vs-dialog--live" : "vs-dialog"}
      ref={ref}
      aria-labelledby="share-title"
      onCancel={onClose}
    >
      {collaboration && (
        <button type="button" className="vs-close" aria-label="Close sharing" onClick={onClose}>
          ×
        </button>
      )}
      {confirm ? (
        <>
          <h2>Delete share link?</h2>
          <p>Downloaded copies stay with their owners.</p>
          <div className="vs-actions">
            <button
              type="button"
              data-destructive=""
              onClick={() => void act({ action: "delete", id: state.id })}
            >
              Delete share link
            </button>
            <button type="button" onClick={() => setConfirm(false)}>
              {translateUi("Cancel")}
            </button>
          </div>
        </>
      ) : collaboration && state.state === "shared" ? (
        <>
          <header className="vs-live-header">
            <h2 id="share-title">
              {translateUi("Share")} {name}
            </h2>
            <p>Snapshot and live collaboration are separate.</p>
          </header>
          <section className="vs-live-snapshot">
            <header>
              <Icon name="link" size={14} />
              <strong>Snapshot link</strong>
              <span>✓ Shared</span>
            </header>
            <p>
              A copy of this file, its images, fonts, scripts and libraries. Update it explicitly
              when you want people to see new changes.
            </p>
            <div className="vs-live-row">
              <a className="vs-url" href={state.url} target="_blank" rel="noreferrer">
                {state.url}
              </a>
              <button type="button" disabled={busy} onClick={() => void act({ action: "status" })}>
                {translateUi("Copy")}
              </button>
            </div>
            <div className="vs-live-row">
              <p>
                Snapshot from{" "}
                {state.snapshot
                  ? new Intl.DateTimeFormat("en-GB", {
                      day: "numeric",
                      month: "short",
                      hour: "2-digit",
                      minute: "2-digit",
                      hour12: false,
                      timeZone: "UTC",
                    }).format(state.snapshot.at)
                  : "the last upload"}
              </p>
              <button
                type="button"
                disabled={busy || !collaboration.state.sharing?.owner}
                onClick={() => void act({ action: "create", id: state.id })}
              >
                Update snapshot
              </button>
              <button
                type="button"
                data-destructive=""
                disabled={busy || !collaboration.state.sharing?.owner}
                onClick={() => void act({ action: "delete", id: state.id })}
              >
                Delete snapshot link
              </button>
            </div>
            <p>
              Delete snapshot link removes the public copy only. It does not change live access or
              remove collaborators.
            </p>
          </section>
        </>
      ) : (
        <ShareDialogBody
          name={name}
          state={state}
          act={(r) => void act(r)}
          close={onClose}
          busy={busy}
          canManage={!collaboration || collaboration.state.sharing?.owner === true}
          openViewer={() => {
            if (state.url) void port.open(state.url).catch(() => setNotice("Viewer unavailable"));
          }}
        />
      )}
      {notice && <p role="status">{notice}</p>}
      {collaboration && !confirm ? <LiveShareSection controller={collaboration} /> : null}
      {collaboration && !confirm && (
        <footer className="vs-live-footer">
          <Icon name="info" size={12} />
          <p>Live access and the snapshot link are managed separately.</p>
          <button type="button" onClick={onClose}>
            {translateUi("Done")}
          </button>
        </footer>
      )}
    </dialog>
  );
}
