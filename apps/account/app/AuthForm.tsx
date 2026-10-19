"use client";
import { brand, copy } from "@vellum/config";
import { useState } from "react";
export function AuthForm({
  deviceCode = "",
  signedIn = false,
}: {
  deviceCode?: string;
  signedIn?: boolean;
}) {
  const [email, setEmail] = useState("");
  const [code, setCode] = useState(deviceCode);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  async function request(path: string, body: unknown) {
    const response = await fetch(`/api/auth${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error("Request failed");
  }
  async function signIn() {
    setBusy(true);
    setStatus("");
    try {
      await request("/sign-in/magic-link", {
        email,
        callbackURL: code ? `/device?user_code=${encodeURIComponent(code)}` : "/device",
      });
      setStatus("✓ Check your email for a sign-in link.");
    } catch {
      setStatus("⊘ Sign-in unavailable. Try again.");
    } finally {
      setBusy(false);
    }
  }
  async function decide(approve: boolean) {
    setBusy(true);
    setStatus("");
    try {
      const response = await fetch(`/api/auth/device?user_code=${encodeURIComponent(code)}`);
      if (!response.ok) throw new Error();
      await request(approve ? "/device/approve" : "/device/deny", { userCode: code });
      setStatus(approve ? "✓ Session connected. Return to your CLI." : "⊘ Request denied.");
    } catch {
      setStatus("⊘ Code unavailable or expired. Request a new code.");
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="account-shell">
      <header>{brand.name}</header>
      <main className={signedIn ? "device-form" : "signin-form"}>
        {signedIn ? (
          <>
            <h1>Connect this session</h1>
            <p>Approve only if this code matches the session you started.</p>
            <label>
              Device code
              <input
                className="device-code"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                autoComplete="off"
                maxLength={191}
              />
            </label>
            <p>This connects the CLI; it does not share your repo.</p>
            <div className="decisions">
              <button type="button" disabled={busy || !code} onClick={() => void decide(true)}>
                Approve
              </button>
              <button
                className="quiet"
                type="button"
                disabled={busy || !code}
                onClick={() => void decide(false)}
              >
                Deny
              </button>
            </div>
          </>
        ) : (
          <>
            <h1>Sign in to {brand.name}</h1>
            <p>Enable share links and cloud sync for files you choose.</p>
            <p className="scope">{copy.syncScopeA05}</p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void signIn();
              }}
            >
              <label>
                Email
                <input
                  type="email"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  autoComplete="email"
                />
              </label>
              <button disabled={busy} type="submit">
                Continue with email
              </button>
            </form>
            <small>Local editing and agents work without an account.</small>
          </>
        )}
        {status && <p role="status">{status}</p>}
      </main>
    </div>
  );
}
