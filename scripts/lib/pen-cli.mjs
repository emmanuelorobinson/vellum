// The headless pen.dev CLI as design-snapshot and design-tokens use it: one `pen interactive` session per
// call on the read-only copy, whose output file is a scratch path that is never saved to.
//
// VELLUM_PEN_CLI overrides the pen command (default: `pen` on PATH, else the pinned npx package).
import { spawnSync } from "node:child_process";
import { join } from "node:path";

const PINNED_CLI = "npx --yes @pen.dev/cli@0.3.11"; // docs/architecture/version-baseline.md C9
const quote = (s) => `'${s.replaceAll("'", `'\\''`)}'`;

function penCommand() {
  // biome-ignore lint/suspicious/noUndeclaredEnvVars: a developer override read at run time, not a Turborepo task input
  const override = process.env.VELLUM_PEN_CLI;
  if (override) return override;
  const onPath = spawnSync("sh", ["-c", "command -v pen"], { encoding: "utf8" });
  return onPath.status === 0 ? "pen" : PINNED_CLI;
}

/** Runs `lines` in one session on `copy`; returns stdout and stderr. Throws with a readable message. */
export function penInteractive({ copy, cache, lines }) {
  const cmd = `${penCommand()} interactive -i ${quote(copy)} -o ${quote(join(cache, "headless-out.pen"))}`;
  const result = spawnSync(cmd, {
    shell: true,
    input: `${[...lines, "exit()"].join("\n")}\n`,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  if (/Authentication required/i.test(output))
    throw new Error(
      'The pen CLI is not signed in. Run "pen login" (or set PEN_CLI_KEY), then run this again.',
    );
  if (result.status !== 0)
    throw new Error(`pen interactive failed (exit ${result.status}):\n${output.slice(-2000)}`);
  return output;
}
