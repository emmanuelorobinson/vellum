/** Desktop turns use the existing native CLI runner and its daemon-owned sessions and Op bridge. */
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import type { NativePrompt } from "@vellum/host-api";

export function promptArguments(
  path: string,
  session: string,
  request: NativePrompt,
  runDir?: string,
) {
  const engine = request.engine ?? { engine: "codex", model: "default" };
  const args = [
    ...(runDir ? ["--run-dir", runDir] : []),
    "--json",
    "prompt",
    "--session",
    session,
    "--engine",
    engine.engine,
    "--model",
    engine.model,
  ];
  if (engine.effort) args.push("--effort", engine.effort);
  if (request.explore)
    args.push(
      "--parallel",
      String(request.explore.count),
      "--mode",
      `explore-${request.explore.vary}`,
    );
  else
    args.push(
      "--parallel",
      String(request.parallel.count),
      "--mode",
      request.parallel.mode.replaceAll("_", "-"),
    );
  const context = request.context.map((c) => `${c.kind}: ${c.label} (${c.value})`).join("\n");
  const skills = request.skills.map((s) => `Use the Vellum skill ${s}.`).join("\n");
  args.push(
    "--",
    path,
    [request.text, context && `Canvas context:\n${context}`, skills].filter(Boolean).join("\n\n"),
  );
  return args;
}

async function program(name: string) {
  const folders = [
    ...(process.env.PATH ?? "").split(delimiter),
    join(homedir(), ".local/bin"),
    "/opt/homebrew/bin",
    "/usr/local/bin",
  ];
  for (const folder of folders) {
    if (!folder) continue;
    const path = join(folder, process.platform === "win32" ? `${name}.exe` : name);
    if (
      await access(path).then(
        () => true,
        () => false,
      )
    )
      return path;
  }
  return null;
}
async function signedIn(path: string, codex: boolean): Promise<boolean> {
  return new Promise((resolve) => {
    const child = spawn(path, codex ? ["login", "status"] : ["auth", "status", "--json"], {
      stdio: "pipe",
      windowsHide: true,
      shell: false,
    });
    let text = "";
    const collect = (chunk: Buffer) => {
      if (text.length < 128_000) text += chunk.toString();
    };
    child.stdout.on("data", collect);
    child.stderr.on("data", collect);
    const timer = setTimeout(() => {
      child.kill();
      resolve(false);
    }, 6000);
    child.on("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      resolve(
        code === 0 &&
          (codex
            ? /Logged in using (ChatGPT|an API key)/.test(text)
            : /"loggedIn"\s*:\s*true/.test(text)),
      );
    });
    child.stdin.end();
  });
}

interface ModelEntry {
  id: string;
  label: string;
  caps: { vision: boolean; fast: boolean; reasoning: boolean; contextTokens: number | null };
}
const modelEntry = (id: string, label: string, reasoning: boolean): ModelEntry => ({
  id,
  label,
  caps: { vision: false, fast: false, reasoning, contextTokens: null },
});
/** The models Codex lists for itself (its own cache), newest first; none when it has not fetched them. */
export async function codexModels(home = homedir()): Promise<ModelEntry[]> {
  try {
    const cache = JSON.parse(await readFile(join(home, ".codex", "models_cache.json"), "utf8")) as {
      models?: { slug?: unknown; display_name?: unknown; visibility?: unknown }[];
    };
    return (cache.models ?? [])
      .filter(
        (m) =>
          m.visibility === "list" &&
          typeof m.slug === "string" &&
          /^[A-Za-z0-9._:/-]{1,120}$/.test(m.slug),
      )
      .map((m) =>
        modelEntry(
          m.slug as string,
          typeof m.display_name === "string" ? m.display_name : (m.slug as string),
          true,
        ),
      );
  } catch {
    return [];
  }
}
/** Claude Code's documented aliases: each follows the latest model of its kind. */
export const CLAUDE_MODELS: readonly ModelEntry[] = [
  modelEntry("fable", "Fable", true),
  modelEntry("opus", "Opus", true),
  modelEntry("sonnet", "Sonnet", true),
];

export class DesktopAgentRuns {
  private running = new Map<
    string,
    { child: ChildProcessWithoutNullStreams; done: Promise<void> }
  >();
  constructor(
    private readonly binary: string,
    private readonly runDir?: string,
    private readonly findProgram = program,
  ) {}
  async engines() {
    const installed = await Promise.all(
      [
        ["codex", "Codex"],
        ["claude-code", "Claude Code"],
      ].map(async ([id, name]) => {
        const path = await this.findProgram(id === "codex" ? "codex" : "claude");
        return {
          id,
          name,
          state: !path
            ? "missing"
            : (await signedIn(path, id === "codex"))
              ? "signed_in"
              : "signed_out",
          command: id === "codex" ? "codex login" : "claude",
          models: path ? (id === "codex" ? await codexModels() : [...CLAUDE_MODELS]) : [],
        };
      }),
    );
    return { builtin: [], installed };
  }
  async send(
    path: string,
    session: string,
    request: NativePrompt,
    live: (turn: string, text: string) => void,
  ): Promise<{ turn: string }> {
    const prior = this.running.get(session);
    if (prior) {
      if (request.queue === "now") this.stop(session);
      else if (request.queue !== "after") throw Error("A turn is already running");
      await prior.done;
    }
    const engine = request.engine?.engine ?? "codex";
    const executable = await this.findProgram(engine === "codex" ? "codex" : "claude");
    if (!executable) throw Error("The installed engine is unavailable");
    const env = {
      ...process.env,
      PATH: `${dirname(executable)}${delimiter}${process.env.PATH ?? ""}`,
      TURBO_TELEMETRY_DISABLED: "1",
      NEXT_TELEMETRY_DISABLED: "1",
    };
    const child = spawn(this.binary, promptArguments(path, session, request, this.runDir), {
      cwd: dirname(path),
      env,
      windowsHide: true,
      shell: false,
      stdio: "pipe",
    });
    let finish = () => {};
    const done = new Promise<void>((resolve) => {
      finish = resolve;
    });
    this.running.set(session, { child, done });
    child.stderr.resume(); // Native diagnostics and account details never cross into the renderer.
    child.stdin.end();
    return new Promise((resolve, reject) => {
      let buffer = "",
        turn = "";
      const timeout = setTimeout(() => {
        this.stop(session);
        reject(Error("The engine did not start a turn"));
      }, 30_000);
      child.stdout.on("data", (chunk: Buffer) => {
        buffer += chunk.toString();
        if (buffer.length > 1024 * 1024) {
          this.stop(session);
          reject(Error("The engine response exceeded its limit"));
          return;
        }
        while (buffer.includes("\n")) {
          const newline = buffer.indexOf("\n");
          const line = buffer.slice(0, newline);
          buffer = buffer.slice(newline + 1);
          try {
            const event = JSON.parse(line);
            if (event.event === "turn_started" && typeof event.turn === "string") {
              turn = event.turn;
              clearTimeout(timeout);
              resolve({ turn });
            }
            if (event.event === "text" && turn && typeof event.text === "string")
              live(turn, event.text);
          } catch {
            /* A partial or invalid protocol line is never displayed as conversation. */
          }
        }
      });
      const ended = () => {
        clearTimeout(timeout);
        this.running.delete(session);
        finish();
        if (!turn) reject(Error("The engine could not start. Check its sign-in and model."));
      };
      child.once("error", ended);
      child.once("exit", ended);
    });
  }
  stop(session: string) {
    const child = this.running.get(session)?.child;
    child?.kill("SIGINT");
    if (child) {
      const timer = setTimeout(() => {
        if (child.exitCode === null) child.kill("SIGKILL");
      }, 3000);
      child.once("exit", () => clearTimeout(timer));
      timer.unref();
    }
  }
  dispose() {
    for (const session of this.running.keys()) this.stop(session);
  }
}
