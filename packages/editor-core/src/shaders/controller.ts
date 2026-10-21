import type { Uniform } from "@vellum/types-ts";
export interface ShaderUniform {
  name: string;
  label?: string;
  kind: "number" | "boolean" | "color" | "vector";
  size?: 2 | 3 | 4;
  default?: Uniform;
}
export type ShaderView =
  | { status: "ready"; uniforms: readonly ShaderUniform[] }
  | { status: "error"; kind: "invalid" | "unsupported" | "missing"; message: string }
  | { status: "loading" | "unavailable" };
export interface ShaderChoice {
  src: string;
  uniforms: Record<string, Uniform>;
}
export interface ShaderHost {
  /** Compile and reflect the bounded shader through ShaderCompiler. Host owns source reads and authorization. */
  inspect?(src: string): Promise<ShaderView>;
  choose?(effect: boolean): Promise<ShaderChoice | null>;
  /** Copy a bundled example into this document's dependencies; return its relative source. */
  importExample?(id: string): Promise<ShaderChoice | null>;
  openExample?(id: string): Promise<void>;
}
export function parseUniform(info: ShaderUniform, raw: string | boolean): Uniform {
  if (info.kind === "boolean") {
    if (![true, false, "true", "false"].includes(raw)) throw Error("Use true or false");
    return raw === true || raw === "true";
  }
  if (info.kind === "color") {
    if (typeof raw !== "string" || !/^#[\da-f]{6}([\da-f]{2})?$/i.test(raw))
      throw Error("Use a hex colour");
    return raw;
  }
  const finite = (s: string) => {
    const n = Number(s);
    if (!s.trim() || !Number.isFinite(n) || Math.abs(n) > 1e6)
      throw Error("Use a finite number between −1000000 and 1000000");
    return n;
  };
  if (info.kind === "number") return finite(String(raw));
  const values = String(raw).split(",").map(finite);
  if (values.length !== info.size) throw Error(`Use ${info.size} comma-separated numbers`);
  return values as Uniform;
}
export function createShaderController(host: ShaderHost) {
  const views = new Map<string, ShaderView>();
  const epochs = new Map<string, number>();
  const listeners = new Set<() => void>();
  const emit = () => {
    for (const listener of listeners) listener();
  };
  return {
    host,
    subscribe(fn: () => void) {
      listeners.add(fn);
      return {
        dispose() {
          listeners.delete(fn);
        },
      };
    },
    view(src: string): ShaderView {
      return views.get(src) ?? { status: host.inspect ? "loading" : "unavailable" };
    },
    async refresh(src: string) {
      if (!host.inspect) return;
      const epoch = (epochs.get(src) ?? 0) + 1;
      epochs.set(src, epoch);
      views.set(src, { status: "loading" });
      emit();
      let view: ShaderView;
      try {
        view = await host.inspect(src);
      } catch {
        view = { status: "error", kind: "missing", message: "Shader source could not be loaded" };
      }
      if (epochs.get(src) !== epoch) return;
      views.set(src, view);
      emit();
    },
  };
}
export type ShaderController = ReturnType<typeof createShaderController>;
