// TEST DATA: scripted native CLI; no real keychain or browser.

import type { AccountRunner } from "@vellum/host-api/native-account";
import { beforeEach, expect, it, vi } from "vitest";

const mock = vi.hoisted(() => ({
  commands: new Map<string, () => Promise<void>>(),
  notice: vi.fn(),
  error: vi.fn(),
  open: vi.fn(),
  report: vi.fn(),
  cancel: undefined as (() => void) | undefined,
}));
vi.mock("vscode", () => ({
  commands: {
    registerCommand: (id: string, action: () => Promise<void>) => {
      mock.commands.set(id, action);
      return { dispose() {} };
    },
  },
  ProgressLocation: { Notification: 15 },
  window: {
    showInformationMessage: mock.notice,
    showErrorMessage: mock.error,
    withProgress: (
      _options: unknown,
      action: (progress: unknown, token: unknown) => Promise<void>,
    ) =>
      action(
        { report: mock.report },
        {
          isCancellationRequested: false,
          onCancellationRequested: (cancel: () => void) => {
            mock.cancel = cancel;
            return { dispose() {} };
          },
        },
      ),
  },
  env: { openExternal: mock.open },
  Uri: { parse: (uri: string) => uri },
}));

import { accountCommands } from "./account";

beforeEach(() => {
  vi.clearAllMocks();
  mock.commands.clear();
  mock.notice.mockResolvedValue(undefined);
  mock.open.mockResolvedValue(true);
});
const waiting = {
  ok: true,
  command: "login",
  state: "waiting",
  user_code: "ABCD-2345",
  verification_url: "https://account.example/device?user_code=ABCD-2345",
  expires_in: 600,
};
it("registers lazy commands, shows the comparison code and opens only the approved public URL", async () => {
  const run = vi.fn<AccountRunner>(async (_command, _signal, emit) => {
    emit(waiting);
    await Promise.resolve();
    emit({ ok: true, command: "login", signed_in: true });
    return 0;
  });
  accountCommands("/bundled/vellum", run);
  expect(run).not.toHaveBeenCalled();
  mock.notice.mockResolvedValueOnce("Open approval page");
  await mock.commands.get("vellum.signIn")?.();
  expect(mock.report).toHaveBeenCalledWith({
    message: "Code: ABCD-2345 · Approve in your browser",
  });
  expect(mock.open).toHaveBeenCalledWith(waiting.verification_url);
  expect(mock.notice).toHaveBeenLastCalledWith("Vellum: signed in.");
});
it("cancels native polling and never opens a stale notification", async () => {
  let choose: (value: string) => void = () => {};
  mock.notice.mockReturnValueOnce(
    new Promise<string>((resolve) => {
      choose = resolve;
    }),
  );
  let aborted = false;
  const run: AccountRunner = async (_command, signal, emit) =>
    new Promise((resolve) => {
      emit(waiting);
      signal.addEventListener(
        "abort",
        () => {
          aborted = true;
          resolve(1);
        },
        { once: true },
      );
    });
  const registrations = accountCommands("/bundled/vellum", run);
  const pending = mock.commands.get("vellum.signIn")?.();
  mock.cancel?.();
  await pending;
  choose("Open approval page");
  await Promise.resolve();
  expect(aborted).toBe(true);
  expect(mock.open).not.toHaveBeenCalled();
  for (const item of registrations) item.dispose();
});
it("reports offline revocation honestly and never displays native diagnostics", async () => {
  accountCommands("/bundled/vellum", async (command, _signal, emit) => {
    if (command === "logout") emit({ ok: true, command, signed_in: false, remote_revoked: false });
    else emit({ ok: false, command, error: { message: "private-canary" } });
    return command === "logout" ? 0 : 1;
  });
  await mock.commands.get("vellum.accountStatus")?.();
  expect(JSON.stringify(mock.notice.mock.calls)).not.toContain("private-canary");
  await mock.commands.get("vellum.signOut")?.();
  expect(mock.notice).toHaveBeenLastCalledWith(
    "Vellum: signed out on this computer; remote session revocation unconfirmed.",
  );
});
