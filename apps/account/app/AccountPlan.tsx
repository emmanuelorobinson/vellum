"use client";
import { brand } from "@vellum/config";
import { useEffect, useState } from "react";
import "./plan.css";

interface AccountData {
  user: { name: string; email: string };
  entitlements: string[];
  sessions: { id: string; expiresAt: number; current: boolean }[];
  shares: {
    id: string;
    revoked: boolean;
    snapshot: { name: string; at: number; incomplete: boolean } | null;
  }[];
}
export function AccountPlanBody({
  data,
  busy,
  notice,
  action,
}: {
  data: AccountData;
  busy: boolean;
  notice: string;
  action: (path: string, method: "POST" | "DELETE") => void;
}) {
  const pro = data.entitlements.includes("share_links");
  return (
    <div className="vp-shell">
      <header>{brand.name}</header>
      <main>
        <h1>Account and plan</h1>
        <p>
          {data.user.name} · {data.user.email}
        </p>
        <div className="vp-plans">
          <section>
            <h2>Free</h2>
            <p>Local files, vector canvas, agents and Git.</p>
            <button type="button" disabled>
              Current local features
            </button>
          </section>
          <section>
            <h2>Pro</h2>
            <p>
              Cloud sync, snapshot links, version history, hosted PR previews and premium design
              systems.
            </p>
            <button
              type="button"
              disabled={busy}
              data-primary=""
              onClick={() => action(pro ? "billing/portal" : "billing/checkout", "POST")}
            >
              {pro ? "Manage plan" : "Get Pro"}
            </button>
          </section>
          <section>
            <h2>Team</h2>
            <p>Live multiplayer, comments, shared libraries and roles.</p>
            <p>Coming in v1.1</p>
          </section>
        </div>
        <div className="vp-management">
          <section>
            <h2>Devices and CLI sessions</h2>
            {data.sessions.length === 0 && <p>No active sessions</p>}
            {data.sessions.map((s) => (
              <div className="vp-row" key={s.id}>
                <span>
                  {s.current ? "This session" : "Signed-in session"}
                  <small>Expires {new Date(s.expiresAt).toLocaleString()}</small>
                </span>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => action(`sessions/${encodeURIComponent(s.id)}`, "DELETE")}
                >
                  Revoke
                </button>
              </div>
            ))}
            <button
              type="button"
              disabled={busy || !pro}
              onClick={() => action("billing/portal", "POST")}
            >
              Billing details
            </button>
          </section>
          <section>
            <h2>Shared files</h2>
            {data.shares.filter((s) => !s.revoked && s.snapshot).length === 0 && (
              <p>No shared files</p>
            )}
            {data.shares
              .filter((s) => !s.revoked && s.snapshot)
              .map((s) => (
                <div className="vp-row" key={s.id}>
                  <a href={`/share/${s.id}`} rel="noreferrer">
                    {s.snapshot?.name}
                  </a>
                  <span>{s.snapshot?.incomplete ? "△ Incomplete" : "View-only snapshot"}</span>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => action(`shares/${s.id}`, "DELETE")}
                  >
                    Delete share link
                  </button>
                </div>
              ))}
          </section>
        </div>
        <p role="status">{notice}</p>
        <footer>
          <button
            type="button"
            disabled={busy || !pro}
            onClick={() => action("billing/portal", "POST")}
          >
            Manage billing
          </button>
          <button
            type="button"
            disabled={busy || !pro}
            onClick={() => action("billing/portal", "POST")}
          >
            Invoices
          </button>
        </footer>
      </main>
    </div>
  );
}
export default function AccountPlan() {
  const [data, setData] = useState<AccountData | null>(null),
    [status, setStatus] = useState("Loading account…"),
    [busy, setBusy] = useState(false),
    [confirm, setConfirm] = useState<string | null>(null);
  const read = async (path: string) => {
    const r = await fetch(`/v1/${path}`, { cache: "no-store" });
    if (!r.ok) throw Error();
    return r.json();
  };
  const load = async () => {
    try {
      const [me, grants, sessions, shares] = await Promise.all([
        read("me"),
        read("entitlements"),
        read("sessions"),
        read("shares"),
      ]);
      setData({
        user: me.user,
        entitlements: grants.entitlements,
        sessions: sessions.sessions,
        shares: shares.shares,
      });
      setStatus("");
    } catch {
      setData(null);
      setStatus("Account unavailable. Sign in or try again when the service is available.");
    }
  };
  useEffect(() => {
    void load();
  }, []);
  const action = async (path: string, method: "POST" | "DELETE") => {
    if (method === "DELETE" && confirm !== path) {
      setConfirm(path);
      return;
    }
    setConfirm(null);
    setBusy(true);
    try {
      const r = await fetch(`/v1/${path}`, {
        method,
        headers: { "content-type": "application/json" },
        body: method === "POST" ? "{}" : undefined,
        redirect: "error",
      });
      if (!r.ok) throw Error();
      if (method === "POST") {
        const v = await r.json(),
          u = new URL(v.url);
        if (u.protocol !== "https:" || u.username || u.password) throw Error();
        window.location.assign(u.href);
      } else await load();
    } catch {
      setStatus("The action could not be completed. Try again.");
    } finally {
      setBusy(false);
    }
  };
  if (!data)
    return (
      <div className="vp-shell">
        <header>{brand.name}</header>
        <main>
          <h1>Account and plan</h1>
          <p>{brand.tagline}</p>
          <p role="status">{status}</p>
          <a href="/sign-in">Sign in</a>
          <button type="button" onClick={() => void load()}>
            Retry
          </button>
        </main>
      </div>
    );
  return (
    <>
      <AccountPlanBody
        data={data}
        busy={busy}
        notice={status}
        action={(p, m) => void action(p, m)}
      />
      {confirm && (
        <div className="vp-confirm" role="alertdialog" aria-label="Confirm revocation">
          <p>
            {confirm.startsWith("shares/")
              ? "Delete share link? Downloaded copies stay with their owners."
              : "Revoke this session? It will need to sign in again."}
          </p>
          <button type="button" onClick={() => void action(confirm, "DELETE")}>
            Revoke
          </button>
          <button type="button" onClick={() => setConfirm(null)}>
            Cancel
          </button>
        </div>
      )}
    </>
  );
}
