import { spawn } from "node:child_process";
import type { AccountState } from "./account-state";

export type AccountCommand = "login" | "whoami" | "logout";
export type AccountRecord = Record<string, unknown>;
export type AccountRunner = (
  command: AccountCommand,
  signal: AbortSignal,
  emit: (record: AccountRecord) => void,
) => Promise<number>;

function verification(record: AccountRecord): AccountState {
  const code = record.user_code;
  if (typeof code !== "string" || !/^[A-Za-z0-9-]{1,191}$/.test(code)) throw Error("invalid");
  if (typeof record.verification_url !== "string" || record.verification_url.length > 4096)
    throw Error("invalid");
  const url = new URL(record.verification_url);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    (url.protocol !== "https:" && !(local && url.protocol === "http:")) ||
    url.username ||
    url.password ||
    url.hash ||
    url.pathname !== "/device" ||
    [...url.searchParams.keys()].join(",") !== "user_code" ||
    url.searchParams.get("user_code") !== code ||
    typeof record.expires_in !== "number" ||
    !Number.isInteger(record.expires_in) ||
    record.expires_in < 1 ||
    record.expires_in > 600
  )
    throw Error("invalid");
  return { kind: "waiting", userCode: code, verificationUrl: url.href };
}

function validUser(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const user = value as Record<string, unknown>;
  return ["id", "email", "name"].every((key) => {
    const field = user[key];
    return typeof field === "string" && field.length <= 512 && !/\p{Cc}/u.test(field);
  });
}

/** Lazy: constructing a host never reads the keychain or contacts an account service. */
export class NativeAccount {
  private current: AccountState = { kind: "idle" };
  private active?: { abort: AbortController; done: Promise<void> };
  private disposed = false;
  private signingOut = false;
  constructor(
    private readonly run: AccountRunner,
    private readonly changed: () => void = () => {},
  ) {}
  get state(): AccountState {
    return { ...this.current };
  }
  private set(value: AccountState) {
    this.current = value;
    this.changed();
  }
  login() {
    return this.command("login");
  }
  refresh() {
    return this.command("whoami");
  }
  async logout() {
    if (this.signingOut || this.disposed) return;
    this.signingOut = true;
    try {
      if (this.active) await this.cancel();
      await this.command("logout");
    } finally {
      this.signingOut = false;
    }
  }
  async cancel() {
    const active = this.active;
    if (!active) return;
    active.abort.abort();
    await active.done;
    if (!this.disposed) this.set({ kind: "idle" });
  }
  dispose() {
    this.disposed = true;
    this.active?.abort.abort();
    this.current = { kind: "idle" };
  }
  private command(command: AccountCommand): Promise<void> {
    if (this.active || this.disposed || (this.signingOut && command !== "logout"))
      return Promise.resolve();
    const abort = new AbortController();
    let finish!: () => void;
    const done = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const active = { abort, done };
    this.active = active;
    this.set({ kind: "busy" });
    let final: AccountState | undefined;
    let invalid = false;
    void (async () => {
      try {
        const code = await this.run(command, abort.signal, (record) => {
          if (abort.signal.aborted || this.disposed || invalid) return;
          try {
            if (record.command !== command || final) throw Error("invalid");
            if (record.ok === false) {
              // CLI errors are fixed text, but never reflect arbitrary child output into a host.
              if (
                command === "whoami" &&
                (record.error as { message?: unknown })?.message === "Signed out"
              )
                final = { kind: "signed-out" };
              else final = { kind: "failed" };
            } else if (record.ok !== true) throw Error("invalid");
            else if (command === "login" && record.state === "waiting")
              this.set(verification(record));
            else if (command === "login" && record.signed_in === true)
              final = { kind: "signed-in" };
            else if (command === "whoami" && validUser(record.user)) final = { kind: "signed-in" };
            else if (
              command === "logout" &&
              record.signed_in === false &&
              typeof record.remote_revoked === "boolean"
            )
              final = { kind: "signed-out", remoteRevoked: record.remote_revoked };
            else throw Error("invalid");
          } catch {
            invalid = true;
            abort.abort();
          }
        });
        if (!this.disposed)
          this.set(
            invalid
              ? { kind: "failed" }
              : abort.signal.aborted
                ? { kind: "idle" }
                : final &&
                    (code === 0 ||
                      (command === "whoami" && code === 1 && final.kind === "signed-out"))
                  ? final
                  : { kind: "failed" },
          );
      } catch {
        if (!this.disposed)
          this.set({ kind: abort.signal.aborted && !invalid ? "idle" : "failed" });
      } finally {
        if (this.active === active) this.active = undefined;
        finish();
      }
    })();
    return done;
  }
}

/** Fixed bundled executable and argv. Raw stdout/stderr never enter renderer state or logs. */
export function accountRunner(binary: string): AccountRunner {
  return (command, signal, emit) =>
    new Promise((resolve, reject) => {
      if (signal.aborted) {
        resolve(1);
        return;
      }
      const child = spawn(binary, [command, "--json"], {
        stdio: ["ignore", "pipe", "ignore"],
        shell: false,
        windowsHide: true,
      });
      let bytes = 0;
      let pending = "";
      let invalid = false;
      const stop = () => child.kill("SIGKILL");
      signal.addEventListener("abort", stop, { once: true });
      const timer = setTimeout(
        () => {
          invalid = true;
          stop();
        },
        command === "login" ? 660_000 : 40_000,
      );
      child.stdout.setEncoding("utf8");
      child.stdout.on("data", (chunk: string) => {
        if (signal.aborted || invalid) return;
        bytes += Buffer.byteLength(chunk);
        if (bytes > 64 * 1024) {
          invalid = true;
          stop();
          return;
        }
        pending += chunk;
        let end = pending.indexOf("\n");
        while (end >= 0) {
          const line = pending.slice(0, end);
          pending = pending.slice(end + 1);
          try {
            const record: unknown = JSON.parse(line);
            if (!record || typeof record !== "object" || Array.isArray(record))
              throw Error("invalid");
            emit(record as AccountRecord);
          } catch {
            invalid = true;
            stop();
            return;
          }
          end = pending.indexOf("\n");
        }
      });
      child.on("error", () => {
        invalid = true;
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        signal.removeEventListener("abort", stop);
        if (invalid || pending.trim()) reject(Error("Account request failed"));
        else resolve(code ?? 1);
      });
    });
}
