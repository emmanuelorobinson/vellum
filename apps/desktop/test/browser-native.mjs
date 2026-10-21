// needs: installed Electron (electron lane); creates a loopback-only fixture server.

import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { build } from "esbuild";

const output = resolve(import.meta.dirname, "../../../target/browser-native.cjs");
await build({
  entryPoints: [resolve(import.meta.dirname, "browser-native.ts")],
  outfile: output,
  bundle: true,
  platform: "node",
  format: "cjs",
  external: ["electron"],
});
const require = createRequire(import.meta.url);
const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;
console.log("Starting Electron BrowserPort acceptance");
const child = spawn(require("electron"), ["--enable-logging=stderr", output], {
  env,
  stdio: "inherit",
});
const timeout = setTimeout(() => {
  console.error("Electron native acceptance timed out before completion.");
  child.kill("SIGKILL");
  process.exitCode = 1;
}, 90000);
child.on("exit", (code) => {
  clearTimeout(timeout);
  process.exitCode = code ?? 1;
});
