// TEST DATA. Resolve the actual graph: app staging must wait for the shared engine.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

test("both viewer hosts wait for the actual WASM build", async () => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const result = await promisify(execFile)(
    `${root}/node_modules/.bin/turbo`,
    ["run", "build", "--filter=@vellum/account", "--filter=@vellum/web", "--dry=json"],
    {
      cwd: root,
      env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
    },
  );
  const graph = JSON.parse(result.stdout);
  for (const app of ["@vellum/web", "@vellum/account"])
    assert.ok(
      graph.tasks.find((t) => t.taskId === `${app}#build`)?.dependencies.includes("//#wasm:build"),
    );
});
