import type { UpdateState } from "@vellum/host-electron/bridge";
export interface UpdateDriver {
  check(): Promise<string | null>;
  /** Must resolve only after manifest, artifact hash and platform signature verification. */
  download(): Promise<void>;
  install(): void;
}
export class Updates {
  state: UpdateState;
  private busy = false;
  constructor(
    private readonly driver: UpdateDriver | null,
    private readonly current: string,
  ) {
    this.state = driver ? { kind: "idle" } : { kind: "unavailable" };
  }
  async check() {
    if (!this.driver || this.busy) return;
    this.busy = true;
    this.state = { kind: "checking" };
    try {
      const version = await this.driver.check();
      const parts = (v: string) => (/^\d+\.\d+\.\d+$/.test(v) ? v.split(".").map(Number) : null);
      const a = version ? parts(version) : null,
        b = parts(this.current);
      const index = a && b ? a.findIndex((v, i) => v !== b[i]) : -1;
      this.state =
        version && a && b && index >= 0 && (a[index] ?? 0) > (b[index] ?? 0)
          ? { kind: "available", version }
          : { kind: "current" };
    } catch {
      this.state = { kind: "failed" };
    } finally {
      this.busy = false;
    }
  }
  async download() {
    if (!this.driver || this.busy || this.state.kind !== "available") return;
    const version = this.state.version;
    this.busy = true;
    this.state = { kind: "downloading" };
    try {
      await this.driver.download();
      this.state = { kind: "ready", version };
    } catch {
      this.state = { kind: "failed" };
    } finally {
      this.busy = false;
    }
  }
  install() {
    if (!this.driver || this.busy || this.state.kind !== "ready") return false;
    this.state = { kind: "idle" };
    this.driver.install();
    return true;
  }
}
