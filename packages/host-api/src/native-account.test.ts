// TEST DATA: no real keychain, account service or browser is used.
import { describe, expect, it } from "vitest";
import { type AccountCommand, type AccountRecord, NativeAccount } from "./native-account";

function fixture() {
  const calls: {
    command: AccountCommand;
    signal: AbortSignal;
    emit: (r: AccountRecord) => void;
    done: (code: number) => void;
  }[] = [];
  const account = new NativeAccount(
    async (command, signal, emit) =>
      new Promise<number>((done) => {
        calls.push({ command, signal, emit, done });
        signal.addEventListener("abort", () => done(1), { once: true });
      }),
  );
  return {
    account,
    calls,
    call: (index: number) => {
      const value = calls[index];
      if (!value) throw Error("missing invocation");
      return value;
    },
  };
}
const waiting = {
  ok: true,
  command: "login",
  state: "waiting",
  user_code: "ABCD-EFGH",
  verification_url: "https://account.example/device?user_code=ABCD-EFGH",
  expires_in: 600,
};

describe("native account lifecycle", () => {
  it("starts idle without reading credentials, projects only public fields and completes login", async () => {
    const { account, calls, call } = fixture();
    expect(account.state).toEqual({ kind: "idle" });
    expect(calls).toHaveLength(0);
    const pending = account.login();
    call(0).emit({ ...waiting, access_token: "canary-do-not-forward" });
    expect(account.state).toEqual({
      kind: "waiting",
      userCode: "ABCD-EFGH",
      verificationUrl: waiting.verification_url,
    });
    expect(JSON.stringify(account.state)).not.toContain("canary");
    call(0).emit({ ok: true, command: "login", signed_in: true });
    call(0).done(0);
    await pending;
    expect(account.state).toEqual({ kind: "signed-in" });
  });
  it("serializes requests, cancels the child before logout and ignores late login records", async () => {
    const { account, calls, call } = fixture();
    const login = account.login();
    await account.login();
    await account.refresh();
    expect(calls).toHaveLength(1);
    const logout = account.logout();
    await login;
    await Promise.resolve();
    expect(call(0).signal.aborted).toBe(true);
    call(0).emit({ ok: true, command: "login", signed_in: true });
    expect(account.state.kind).not.toBe("signed-in");
    expect(call(1).command).toBe("logout");
    call(1).emit({ ok: true, command: "logout", signed_in: false, remote_revoked: false });
    call(1).done(0);
    await logout;
    expect(account.state).toEqual({ kind: "signed-out", remoteRevoked: false });
  });
  it("fails closed on unsafe browser URLs, malformed records and successful exit without a result", async () => {
    for (const record of [
      { ...waiting, verification_url: "file:///etc/passwd" },
      { ...waiting, verification_url: "https://account.example/device?access_token=secret" },
      { ...waiting, user_code: "code\nforged" },
      { ok: true, command: "whoami", user: { email: "forged@example.test" } },
    ]) {
      const { account, call } = fixture();
      const login = account.login();
      call(0).emit(record);
      call(0).done(0);
      await login;
      expect(account.state).toEqual({ kind: "failed" });
    }
  });
  it("makes cancellation and disposal final for stale callbacks", async () => {
    const { account, calls, call } = fixture();
    const login = account.login();
    await account.cancel();
    call(0).emit(waiting);
    await login;
    expect(account.state).toEqual({ kind: "idle" });
    account.dispose();
    await account.login();
    expect(calls).toHaveLength(1);
  });
  it("does not forward arbitrary CLI errors or user fields", async () => {
    const { account, call } = fixture();
    const request = account.refresh();
    call(0).emit({ ok: false, command: "whoami", error: { message: "private-canary" } });
    call(0).done(1);
    await request;
    expect(account.state).toEqual({ kind: "failed" });
    expect(JSON.stringify(account.state)).not.toContain("private-canary");
  });
});

it("logout ignores duplicate clicks while its request is pending", async () => {
  const { account, calls, call } = fixture();
  const first = account.logout();
  const second = account.logout();
  expect(call(0).signal.aborted).toBe(false);
  call(0).emit({ ok: true, command: "logout", signed_in: false, remote_revoked: true });
  call(0).done(0);
  await Promise.all([first, second]);
  expect(calls).toHaveLength(1);
});

it("rejects malformed identity payloads and a failed process after a successful logout record", async () => {
  for (const user of [[], {}, { id: "person", email: "person@example.test" }]) {
    const { account, call } = fixture();
    const request = account.refresh();
    call(0).emit({ ok: true, command: "whoami", user });
    call(0).done(0);
    await request;
    expect(account.state).toEqual({ kind: "failed" });
  }
  const { account, call } = fixture();
  const request = account.logout();
  call(0).emit({ ok: true, command: "logout", signed_in: false, remote_revoked: true });
  call(0).done(1);
  await request;
  expect(account.state).toEqual({ kind: "failed" });
});
