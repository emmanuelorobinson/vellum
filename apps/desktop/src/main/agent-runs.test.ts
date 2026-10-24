// TEST DATA: native CLI boundary; no installed engine, account or network is used.
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { NativePrompt } from "@vellum/host-api";
import { expect, it } from "vitest";
import { DesktopAgentRuns, promptArguments } from "./agent-runs";

const request: NativePrompt = {
  text: "TEST DATA $(touch should-not-run)",
  engine: { engine: "codex", model: "default" },
  context: [],
  skills: [],
  parallel: { count: 1, mode: "split" },
  explore: null,
  queue: null,
};
it("passes prompt text as one positional argument and preserves the session and chosen mode", () => {
  const args = promptArguments(
    "/work/design.vellum",
    "SESSION",
    { ...request, explore: { count: 3, vary: "style" } },
    "/private/run",
  );
  expect(args.slice(0, 8)).toEqual([
    "--run-dir",
    "/private/run",
    "--json",
    "prompt",
    "--session",
    "SESSION",
    "--engine",
    "codex",
  ]);
  expect(args.slice(-3, -1)).toEqual(["--", "/work/design.vellum"]);
  // The request stays the first thing the engine reads, as one argument, followed by where the design guidance is.
  expect(args.at(-1)?.startsWith(request.text)).toBe(true);
  expect(args.at(-1)).toContain("design-craft.md");
  expect(args).toContain("explore-style");
});
it.skipIf(process.platform === "win32")(
  "returns the daemon turn id, streams text, and suppresses native diagnostics",
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "vellum-native-run-"));
    const file = join(dir, "runner");
    await writeFile(
      file,
      `#!/usr/bin/env node\nprocess.stderr.write('TEST DATA private diagnostic');\nconsole.log(JSON.stringify({event:'turn_started',turn:'TURN'}));\nsetTimeout(()=>console.log(JSON.stringify({event:'text',text:'TEST DATA response'})),20);\n`,
    );
    await chmod(file, 0o700);
    const runs = new DesktopAgentRuns(file, undefined, async () => process.execPath);
    try {
      let text = "";
      const reply = await runs.send(
        join(dir, "design.vellum"),
        "SESSION",
        request,
        (_turn, chunk) => {
          text += chunk;
        },
      );
      expect(reply).toEqual({ turn: "TURN" });
      await expect.poll(() => text).toBe("TEST DATA response");
    } finally {
      runs.dispose();
      await rm(dir, { recursive: true, force: true });
    }
  },
);
