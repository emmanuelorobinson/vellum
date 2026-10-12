// Minimal Chrome DevTools Protocol driver for the spike harness: launch Chrome, open a page,
// evaluate, dispatch input. Needs a local Chrome (VELLUM_CHROME, else the macOS default path).

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const DEFAULT_CHROME = {
  darwin: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  linux: "/usr/bin/google-chrome",
  win32: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
};

export function chromePath() {
  const path = process.env.VELLUM_CHROME ?? DEFAULT_CHROME[process.platform];
  return path && existsSync(path) ? path : null;
}

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

export async function launchChrome({
  headless = true,
  width = 1280,
  height = 860,
  extraArgs = [],
} = {}) {
  const exe = chromePath();
  if (!exe) throw new Error("Chrome not found; set VELLUM_CHROME");
  const profile = mkdtempSync(join(tmpdir(), "vellum-spike-chrome-"));
  const args = [
    `--user-data-dir=${profile}`,
    "--remote-debugging-port=0",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--disable-backgrounding-occluded-windows",
    // Uncapped frames, so frame time measures the renderer rather than the display's refresh rate.
    "--disable-gpu-vsync",
    "--disable-frame-rate-limit",
    "--ignore-gpu-blocklist",
    `--window-size=${width},${height}`,
    ...(headless ? ["--headless=new"] : []),
    ...extraArgs,
    "about:blank",
  ];
  const proc = spawn(exe, args, { stdio: "ignore" });
  const portFile = join(profile, "DevToolsActivePort");
  for (let i = 0; i < 200 && !existsSync(portFile); i++) await sleep(50);
  if (!existsSync(portFile)) throw new Error("Chrome did not open a DevTools port");
  await sleep(50);
  const [port] = readFileSync(portFile, "utf8").split("\n");
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const page = targets.find((t) => t.type === "page");
  const cdp = await connect(page.webSocketDebuggerUrl);
  return {
    cdp,
    async close() {
      cdp.close();
      proc.kill();
      await new Promise((ok) => (proc.exitCode !== null ? ok() : proc.once("exit", ok)));
      rmSync(profile, { recursive: true, force: true });
    },
  };
}

function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  const listeners = new Map();
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { ok, fail } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) fail(new Error(`${msg.error.message} (${msg.error.code})`));
      else ok(msg.result);
    } else if (msg.method) {
      for (const fn of listeners.get(msg.method) ?? []) fn(msg.params);
    }
  };
  const api = {
    send(method, params = {}) {
      return new Promise((ok, fail) => {
        pending.set(++id, { ok, fail });
        ws.send(JSON.stringify({ id, method, params }));
      });
    },
    on(method, fn) {
      listeners.set(method, [...(listeners.get(method) ?? []), fn]);
    },
    once(method) {
      return new Promise((ok) => {
        const fn = (p) => {
          listeners.set(
            method,
            (listeners.get(method) ?? []).filter((f) => f !== fn),
          );
          ok(p);
        };
        api.on(method, fn);
      });
    },
    /** Evaluates `expression` (awaiting promises) and returns its JSON value; throws on exceptions. */
    async evaluate(expression) {
      const result = await api.send("Runtime.evaluate", {
        expression,
        awaitPromise: true,
        returnByValue: true,
      });
      if (result.exceptionDetails) {
        const d = result.exceptionDetails;
        throw new Error(d.exception?.description ?? d.text);
      }
      return result.result.value;
    },
    close() {
      ws.close();
    },
  };
  return new Promise((ok, fail) => {
    ws.onopen = () => ok(api);
    ws.onerror = () => fail(new Error(`could not connect to ${url}`));
  });
}

/** Navigates and waits for the spike page's `window.__spike` to resolve; returns its summary. */
export async function openSpike(cdp, url) {
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  const loaded = cdp.once("Page.loadEventFired");
  await cdp.send("Page.navigate", { url });
  await loaded;
  for (let i = 0; i < 200; i++) {
    if (await cdp.evaluate("typeof window.__spike === 'object'")) break;
    await sleep(25);
  }
  return cdp.evaluate(
    "window.__spike.then(s => ({ renderer: s.renderer, nodeCount: s.nodeCount, firstFrameMs: s.firstFrameMs, glRenderer: s.glRenderer, width: s.width, height: s.height }))",
  );
}

export function dataUrlToBuffer(dataUrl) {
  return Buffer.from(dataUrl.slice(dataUrl.indexOf(",") + 1), "base64");
}
