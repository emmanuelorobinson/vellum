// A minimal headless Chrome driver over the DevTools protocol for repo scripts (the layout oracle).
// It talks through --remote-debugging-pipe, so Chrome opens no TCP port. Chrome comes from
// VELLUM_CHROME, else the platform's usual install path.

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const DEFAULT_CHROME = {
  darwin: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
  linux: ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium"],
  win32: ["C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"],
};

/** The Chrome binary to use, or null when there is none. */
export function chromePath(env = process.env) {
  if (env.VELLUM_CHROME) return existsSync(env.VELLUM_CHROME) ? env.VELLUM_CHROME : null;
  return (DEFAULT_CHROME[process.platform] ?? []).find((p) => existsSync(p)) ?? null;
}

/**
 * Starts headless Chrome and opens one page. Returns `{ version, send, evaluate, navigate, close }`;
 * `send` and `evaluate` act on that page.
 */
export async function launchChrome({ width = 1280, height = 860 } = {}) {
  const exe = chromePath();
  if (!exe) throw new Error("Chrome not found: install Chrome or set VELLUM_CHROME to its binary");
  const profile = mkdtempSync(join(tmpdir(), "vellum-chrome-"));
  const args = [
    "--headless=new",
    "--remote-debugging-pipe",
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-gpu",
    "--hide-scrollbars",
    "--force-device-scale-factor=1",
    `--window-size=${width},${height}`,
    // GitHub's Linux runners cannot use Chrome's user-namespace sandbox. Local runs keep it.
    ...(process.env.CI ? ["--no-sandbox"] : []),
    "about:blank",
  ];
  // fd 3 carries our messages to Chrome, fd 4 Chrome's replies, each NUL-terminated JSON.
  const proc = spawn(exe, args, { stdio: ["ignore", "ignore", "pipe", "pipe", "pipe"] });
  let stderr = "";
  proc.stdio[2].on("data", (chunk) => {
    stderr = (stderr + chunk).slice(-4000);
  });
  const toChrome = proc.stdio[3];
  const fromChrome = proc.stdio[4];

  let nextId = 0;
  const pending = new Map();
  const listeners = [];
  let buffer = "";
  fromChrome.on("data", (chunk) => {
    buffer += chunk;
    let end = buffer.indexOf("\0");
    while (end !== -1) {
      const msg = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      end = buffer.indexOf("\0");
      if (msg.id !== undefined && pending.has(msg.id)) {
        const { ok, fail, method } = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error) fail(new Error(`${method}: ${msg.error.message}`));
        else ok(msg.result);
      } else if (msg.method) {
        for (const fn of [...listeners]) fn(msg);
      }
    }
  });
  const exited = new Promise((ok) => proc.once("exit", ok));
  exited.then(() => {
    for (const { fail } of pending.values()) fail(new Error(`Chrome exited.\n${stderr}`));
    pending.clear();
  });

  const raw = (method, params = {}, sessionId) =>
    new Promise((ok, fail) => {
      const id = ++nextId;
      pending.set(id, { ok, fail, method });
      toChrome.write(
        `${JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) })}\0`,
      );
    });

  const { product } = await raw("Browser.getVersion");
  const { targetId } = await raw("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await raw("Target.attachToTarget", { targetId, flatten: true });
  const send = (method, params) => raw(method, params, sessionId);
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });

  const once = (method) =>
    new Promise((ok) => {
      const fn = (msg) => {
        if (msg.method === method && msg.sessionId === sessionId) {
          listeners.splice(listeners.indexOf(fn), 1);
          ok(msg.params);
        }
      };
      listeners.push(fn);
    });

  return {
    /** Chrome's version, such as "155.0.8059.39". */
    version: product.replace(/^[^/]*\//, ""),
    send,
    /** Loads `html` as the page's document and waits for its load event. */
    async setContent(html) {
      const loaded = once("Page.loadEventFired");
      await send("Page.navigate", {
        url: `data:text/html;base64,${Buffer.from(html).toString("base64")}`,
      });
      await loaded;
    },
    /** Evaluates `expression` in the page and returns its JSON value; throws on an exception. */
    async evaluate(expression) {
      const result = await send("Runtime.evaluate", { expression, returnByValue: true });
      if (result.exceptionDetails) {
        const d = result.exceptionDetails;
        throw new Error(d.exception?.description ?? d.text);
      }
      return result.result.value;
    },
    async close() {
      if (proc.exitCode === null) {
        raw("Browser.close").catch(() => {});
        const timer = setTimeout(() => proc.kill(), 2000);
        await exited;
        clearTimeout(timer);
      }
      rmSync(profile, { recursive: true, force: true });
    },
  };
}
