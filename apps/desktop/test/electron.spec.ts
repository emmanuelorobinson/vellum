// TEST DATA. Tagged electron lane: real native binary, GUI session and built desktop bundle required.

import { existsSync } from "node:fs";
import {
  chmod,
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { _electron, type ElectronApplication } from "playwright";

let app: ElectronApplication;
let directory: string;
test.beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), "vd-"));
  for (const folder of ["one", "two"]) await mkdir(join(directory, folder));
  for (const file of ["a", "b", "c"])
    await copyFile(
      resolve("../../fixtures/documents/shapes.vellum"),
      join(directory, "one", `${file}.vellum`),
    );
  app = await _electron.launch({
    executablePath: process.env.VELLUM_ELECTRON_EXECUTABLE,
    args: [
      resolve("dist/main/index.cjs"),
      "--user-data",
      join(directory, "profile"),
      "--run-dir",
      join(directory, "run"),
    ],
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
});
test.afterEach(async () => {
  if (app) await app.close();
  // The shared Rust daemon is not owned by the host bridge; only stop this test's isolated run dir.
  const { spawnSync } = await import("node:child_process");
  spawnSync(
    resolve("../../target/debug/vellum"),
    ["--run-dir", join(directory, "run"), "daemon", "stop"],
    { stdio: "ignore" },
  );
  await rm(directory, { recursive: true, force: true });
});
async function menu(id: string) {
  await expect
    .poll(() =>
      app.evaluate(
        ({ Menu }, id) => Menu.getApplicationMenu()?.getMenuItemById(id)?.enabled === true,
        id,
      ),
    )
    .toBe(true);
  await app.evaluate(({ Menu }, id) => {
    const item = Menu.getApplicationMenu()?.getMenuItemById(id);
    if (!item?.enabled) throw Error(`disabled menu ${id}`);
    item.click();
  }, id);
}
async function dialog(paths: string[]) {
  await app.evaluate(({ dialog }, paths) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: paths });
  }, paths);
}
test("@electron folder windows, tabs, native menus and sidecar", async () => {
  const initial = await app.firstWindow();
  await expect(initial.getByRole("button", { name: "Open a folder", exact: true })).toBeVisible();
  await dialog([join(directory, "one")]);
  await initial.getByRole("button", { name: "Open a folder", exact: true }).click();
  await expect.poll(() => app.windows().length).toBe(1);
  const page = app.windows()[0];
  if (!page) throw Error("folder window missing");
  await expect(page.getByRole("heading", { name: "one", exact: true })).toBeVisible();
  await dialog([join(directory, "one", "a.vellum"), join(directory, "one", "b.vellum")]);
  await menu("file.open");
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toBeVisible();
  await expect(page.getByRole("tab", { name: /b.vellum/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  // The active tab's engine loads first (the tab bar moves into the editor once it has).
  await expect(page.locator("[data-content]").first()).toHaveAttribute("data-content", "ready", {
    timeout: 30_000,
  });
  await page.getByRole("tab", { name: /a.vellum/ }).click();
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect
    .poll(() => page.evaluate(() => window.vellumDesktop.request({ type: "daemon" })), {
      timeout: 20_000,
    })
    .toEqual({ ok: true, value: true });
  // The tab's engine is a live replica of the daemon's (ADR-033): the canvas draws, an agent's change
  // arrives as unsaved work, and Save writes it.
  await expect(page.locator("[data-content]").first()).toHaveAttribute("data-content", "ready", {
    timeout: 30_000,
  });
  const tabState = () =>
    page.evaluate(async () => {
      const reply = await window.vellumDesktop.request({ type: "state" });
      if (!reply.ok) throw Error("missing state");
      const state = reply.value as {
        problem: string;
        active: string;
        tabs: { id: string; dirty: boolean }[];
      };
      return {
        problem: state.problem,
        dirty: state.tabs.find((t) => t.id === state.active)?.dirty,
      };
    });
  await expect.poll(tabState).toEqual({ problem: "", dirty: false });
  // The canonical path: macOS temporary folders are reached through a /var symlink.
  const file = await realpath(join(directory, "one", "a.vellum"));
  const { spawnSync } = await import("node:child_process");
  const agent = spawnSync(
    resolve("dist/bin", process.platform === "win32" ? "vellum.exe" : "vellum"),
    [
      "--run-dir",
      join(directory, "run"),
      "tool",
      "update_nodes",
      "--document",
      file,
      "--updates",
      JSON.stringify([{ id: "frame00001", set: { name: "By an agent" } }]),
    ],
    { cwd: dirname(file), encoding: "utf8" },
  );
  expect(agent.status, agent.stderr).toBe(0);
  await expect.poll(tabState).toEqual({ problem: "", dirty: true });
  expect(await readFile(file, "utf8")).not.toContain("By an agent");
  await menu("file.save");
  await expect.poll(() => readFile(file, "utf8")).toContain("By an agent");
  await expect.poll(tabState).toEqual({ problem: "", dirty: false });
  await expect(
    page.evaluate(() => window.vellumDesktop.request({ type: "pauseAll" })),
  ).resolves.toEqual({ ok: true });
  const created = join(directory, "one", "created.vellum");
  await app.evaluate(({ dialog }, filePath) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath });
  }, created);
  await menu("file.new");
  await expect(page.getByRole("tab", { name: /created.vellum/ })).toBeVisible();
  expect(JSON.parse(await readFile(created, "utf8")).meta.formatVersion).toBe(1);
  await menu("view.layers");
  await expect(page.getByRole("tab", { name: /Layers/ })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: "Close a.vellum" }).click();
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toHaveCount(0);
  await dialog([join(directory, "one")]);
  await menu("folder.open");
  await expect.poll(() => app.windows().length).toBe(1);
  await dialog([join(directory, "two")]);
  await menu("folder.open");
  await expect.poll(() => app.windows().length).toBe(2);
  const labels = await app.evaluate(({ Menu }) =>
    Menu.getApplicationMenu()?.items.map((item) => item.label),
  );
  for (const label of ["File", "Edit", "View", "Object", "Arrange", "Agents", "Help"])
    expect(labels).toContain(label);
  const other = app.windows().find((window) => window !== page);
  if (!other) throw Error("second folder window missing");
  const firstState = await page.evaluate(() => window.vellumDesktop.request({ type: "state" }));
  if (!firstState.ok) throw Error("missing state");
  const tabId = (firstState.value as { tabs: { id: string }[] }).tabs[0]?.id;
  if (!tabId) throw Error("missing tab");
  expect(
    await other.evaluate(
      (tab) => window.vellumDesktop.request({ type: "readDocument", tab }),
      tabId,
    ),
  ).toMatchObject({ ok: false });
  expect(
    await page.evaluate(
      (tab) => window.vellumDesktop.request({ type: "readDocument", tab }),
      tabId,
    ),
  ).toMatchObject({ ok: true });
});
test("@electron renderer cannot reach Node, filesystem, network or daemon socket", async () => {
  const page = await app.firstWindow();
  expect(await page.evaluate(() => [typeof require, typeof process, typeof Buffer])).toEqual([
    "undefined",
    "undefined",
    "undefined",
  ]);
  const denied = await page.evaluate(async () => {
    const api = window.vellumDesktop;
    const calls = await Promise.all([
      api.request({ type: "readDocument", tab: "not-granted" }),
      // @ts-expect-error hostile renderer input
      api.request({ type: "read", path: "/etc/passwd" }),
      // @ts-expect-error hostile renderer input
      api.request({ type: "rpc", method: "execute" }),
    ]);
    const blocked = await Promise.all(
      ["file:///etc/passwd", "http://127.0.0.1:1", "vellum-app://editor/daemon.json"].map(
        async (url) => {
          try {
            const reply = await fetch(url);
            return !reply.ok;
          } catch {
            return true;
          }
        },
      ),
    );
    return { keys: Object.keys(api).sort(), calls, blocked };
  });
  expect(denied.keys).toEqual(["request", "subscribe"]);
  expect(denied.calls.every((r) => !r.ok)).toBe(true);
  expect(denied.blocked).toEqual([true, true, true]);
  if (process.platform === "darwin" || process.platform === "win32") {
    const sandboxed = await app.evaluate(({ app }) =>
      app
        .getAppMetrics()
        .filter((metric) => metric.type === "Tab")
        .map((metric) => metric.sandboxed),
    );
    expect(sandboxed.length).toBeGreaterThan(0);
    expect(sandboxed.every(Boolean)).toBe(true);
  }
  expect(await page.evaluate(() => window.open("https://example.org"))).toBe(null);
  const forged = await app.evaluate(async ({ BrowserWindow }, preload) => {
    const rogue = new BrowserWindow({
      show: false,
      webPreferences: { preload, sandbox: true, contextIsolation: true, nodeIntegration: false },
    });
    try {
      await rogue.loadURL("vellum-app://editor/index.html");
      return await rogue.webContents.executeJavaScript(
        "window.vellumDesktop.request({type:'state'})",
      );
    } finally {
      rogue.destroy();
    }
  }, resolve("dist/preload/index.cjs"));
  expect(forged).toMatchObject({ ok: false });
});

test("@electron tab keyboard navigation and changed-file capability revocation", async () => {
  const initial = await app.firstWindow();
  await dialog([join(directory, "one")]);
  await initial.getByRole("button", { name: "Open a folder", exact: true }).click();
  await expect.poll(() => app.windows().some((page) => page !== initial)).toBe(true);
  const page = app.windows().find((page) => page !== initial);
  if (!page) throw Error("missing folder window");
  await dialog([join(directory, "one", "a.vellum"), join(directory, "one", "b.vellum")]);
  await menu("file.open");
  const second = page.getByRole("tab", { name: /b.vellum/ });
  await expect(second).toHaveAttribute("aria-selected", "true");
  // Tabs move into the editor once its engine has loaded; navigate after that.
  await expect(page.locator("[data-content]").first()).toHaveAttribute("data-content", "ready", {
    timeout: 30_000,
  });
  await second.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toBeFocused();
  const state = await page.evaluate(() => window.vellumDesktop.request({ type: "state" }));
  if (!state.ok) throw Error("missing state");
  const id = (state.value as { active: string }).active;
  const file = join(directory, "one", "a.vellum");
  await writeFile(join(directory, "outside.vellum"), "TEST DATA: private outside-file sentinel");
  await rm(file);
  await symlink(join(directory, "outside.vellum"), file);
  expect(
    await page.evaluate((tab) => window.vellumDesktop.request({ type: "readDocument", tab }), id),
  ).toMatchObject({ ok: false });
  await page.getByRole("button", { name: "Close a.vellum" }).click();
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toHaveCount(0);
  expect(
    await page.evaluate((tab) => window.vellumDesktop.request({ type: "readDocument", tab }), id),
  ).toMatchObject({ ok: false });
});

for (const appearance of ["light", "dark"] as const)
  test(`@electron @design A03 document tabs · ${appearance}`, async () => {
    const reference = resolve(`../../docs/design/snapshots/A03-${appearance}-1280.png`);
    test.skip(!existsSync(reference), "Needs the exported A03 design snapshot PNGs");
    const initial = await app.firstWindow();
    await dialog([join(directory, "one")]);
    await initial.getByRole("button", { name: "Open a folder", exact: true }).click();
    await expect.poll(() => app.windows().some((page) => page !== initial)).toBe(true);
    const page = app.windows().find((page) => page !== initial);
    if (!page) throw Error("missing folder window");
    await dialog([
      join(directory, "one", "a.vellum"),
      join(directory, "one", "b.vellum"),
      join(directory, "one", "c.vellum"),
    ]);
    await menu("file.open");
    await expect(page.getByRole("tab", { name: /c.vellum/ })).toBeVisible();
    // TEST DATA: A03's runtime states are injected at the native event boundary, never supplied by production.
    await app.evaluate(({ BrowserWindow }, appearance) => {
      const window = BrowserWindow.getAllWindows()[0];
      if (!window) throw Error("missing window");
      window.webContents.send("vellum:desktop:event", {
        type: "state",
        state: {
          folderName: "TEST DATA",
          active: "1",
          tabs: [
            { id: "1", name: "juniper.vellum", dirty: false, running: false },
            { id: "2", name: "settings.vellum", dirty: true, running: false },
            { id: "3", name: "components.vellum", dirty: false, running: true },
          ],
          appearance,
          platform: "mac",
          daemon: true,
          problem: "document-runtime-unavailable",
        },
      });
    }, appearance);
    await expect(page.getByRole("tab", { name: /settings.vellum/ })).toContainText("Unsaved");
    await page.evaluate(() => document.fonts.ready);
    await page.locator(".desktop-docbar").evaluate((el) => {
      for (const animation of el.getAnimations()) animation.finish();
    });
    const actual = await page
      .locator(".desktop-docbar")
      .screenshot({ animations: "disabled", scale: "css" });
    const expected = await readFile(reference);
    const result = await page.evaluate(
      async ({ actual, expected }) => {
        const load = async (source: string) => {
          const image = new Image();
          image.src = `data:image/png;base64,${source}`;
          await image.decode();
          return image;
        };
        const [a, e] = await Promise.all([load(actual), load(expected)]);
        const read = (img: HTMLImageElement, x: number, y: number) => {
          const canvas = new OffscreenCanvas(808, 40);
          const g = canvas.getContext("2d");
          if (!g) throw Error("canvas unavailable");
          g.drawImage(img, x, y, 808, 40, 0, 0, 808, 40);
          return g.getImageData(0, 0, 808, 40).data;
        };
        const ad = read(a, 0, 0),
          ed = read(e, 8, 32);
        let different = 0;
        for (let index = 0; index < ad.length; index += 4)
          if (
            [0, 1, 2, 3].some(
              (channel) => Math.abs((ad[index + channel] ?? 0) - (ed[index + channel] ?? 0)) > 48,
            )
          )
            different++;
        const sheet = new OffscreenCanvas(808, 128);
        const g = sheet.getContext("2d");
        if (!g) throw Error("canvas unavailable");
        g.drawImage(e, 8, 32, 808, 40, 0, 0, 808, 40);
        g.drawImage(a, 0, 44);
        const diff = new Uint8ClampedArray(ad.length);
        for (let index = 0; index < ad.length; index += 4) {
          const delta = Math.max(
            ...[0, 1, 2].map((channel) =>
              Math.abs((ad[index + channel] ?? 0) - (ed[index + channel] ?? 0)),
            ),
          );
          diff[index] = delta;
          diff[index + 2] = delta;
          diff[index + 3] = 255;
        }
        g.putImageData(new ImageData(diff, 808, 40), 0, 88);
        const blob = await sheet.convertToBlob();
        const png = btoa(
          Array.from(new Uint8Array(await blob.arrayBuffer()), (byte) =>
            String.fromCharCode(byte),
          ).join(""),
        );
        return { fraction: different / (808 * 40), width: a.width, height: a.height, png };
      },
      { actual: actual.toString("base64"), expected: expected.toString("base64") },
    );
    const output = resolve("../../target/screen-goldens");
    await mkdir(output, { recursive: true });
    await writeFile(join(output, `A03-${appearance}-desktop-tabs.png`), actual);
    await writeFile(
      join(output, `A03-${appearance}-compare.png`),
      Buffer.from(result.png, "base64"),
    );
    await page.screenshot({
      path: join(output, `A03-${appearance}-desktop-window.png`),
      scale: "css",
    });
    expect(result.width).toBe(808);
    expect(result.height).toBe(40);
    expect(result.fraction, `A03 tabs ${appearance} difference`).toBeLessThanOrEqual(0.01);
  });

test("@electron desktop Home routes, persistent draft and native Save As", async () => {
  const first = await app.firstWindow();
  await expect(first.getByText("Design where your code lives.", { exact: true })).toBeVisible();
  await first.getByRole("button", { name: "Agents", exact: true }).click();
  await expect(first.getByRole("heading", { name: "Agents and daemon" })).toBeVisible();
  await expect(first.getByText("Usage this week · unavailable", { exact: true })).toBeVisible();
  await first.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(first.getByRole("button", { name: "Continue in browser" })).toBeEnabled();
  await expect(first.getByRole("button", { name: "Check account" })).toBeEnabled();
  await first.getByRole("button", { name: "Home", exact: true }).click();
  await first.getByRole("button", { name: "New file", exact: true }).click();
  await expect.poll(() => app.windows().some((p) => p !== first)).toBe(true);
  const page = app.windows().find((p) => p !== first);
  if (!page) throw Error("missing draft window");
  await expect(page.getByRole("tab", { name: /\.vellum/ })).toBeVisible();
  await menu("home");
  await page.getByRole("tab", { name: "Drafts", exact: true }).click();
  await expect(page.getByRole("button", { name: "Save As…", exact: true })).toBeVisible();
  const saved = join(directory, "one", "draft.vellum");
  await app.evaluate(({ dialog }, filePath) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath });
  }, saved);
  await page.getByRole("button", { name: "Save As…", exact: true }).click();
  await expect.poll(() => existsSync(saved)).toBe(true);
  expect(JSON.parse(await readFile(saved, "utf8")).meta.formatVersion).toBe(1);
  await expect
    .poll(
      async () =>
        JSON.parse(await readFile(join(directory, "profile", "home.json"), "utf8")).files[0]?.kind,
    )
    .toBe("file");
});

for (const unindexed of [false, true])
  test(`@electron draft survives kill -9 ${unindexed ? "without metadata" : "with metadata"} and is offered from Home after restart`, async () => {
    const first = await app.firstWindow();
    await first.getByRole("button", { name: "New file", exact: true }).click();
    await expect
      .poll(async () => {
        try {
          return JSON.parse(await readFile(join(directory, "profile", "home.json"), "utf8"))
            .files[0]?.kind;
        } catch {
          return null;
        }
      })
      .toBe("draft");
    const metadata = JSON.parse(await readFile(join(directory, "profile", "home.json"), "utf8"));
    const draft = metadata.files[0].path;
    const bytes = await readFile(draft);
    const child = app.process();
    child.kill("SIGKILL");
    await expect.poll(() => child.exitCode !== null || child.signalCode !== null).toBe(true);
    if (unindexed) await rm(join(directory, "profile", "home.json"));
    app = await _electron.launch({
      executablePath: process.env.VELLUM_ELECTRON_EXECUTABLE,
      args: [
        resolve("dist/main/index.cjs"),
        "--user-data",
        join(directory, "profile"),
        "--run-dir",
        join(directory, "run"),
      ],
      env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
    });
    const page = await app.firstWindow();
    await page.getByRole("tab", { name: "Drafts", exact: true }).click();
    await expect(page.getByRole("button", { name: "Save As…", exact: true })).toBeVisible();
    expect(await readFile(draft)).toEqual(bytes);
    expect(metadata.files[0].kind).toBe("draft");
  });

for (const appearance of ["light", "dark"] as const)
  for (const screen of ["A01", "A02", "A04", "A05", "A05-device", "A06"] as const)
    test(`@electron @design ${screen} desktop · ${appearance}`, async () => {
      test.skip(
        !existsSync(resolve(`../../docs/design/snapshots/${screen}-${appearance}-1280.png`)),
        "Needs the exported desktop design snapshot PNGs",
      );
      const page = await app.firstWindow();
      await expect(page.getByRole("button", { name: "New file", exact: true })).toBeVisible();
      await expect
        .poll(() => page.evaluate(() => window.vellumDesktop.request({ type: "daemon" })))
        .toEqual({ ok: true, value: true });
      await app.evaluate(({ nativeTheme }, appearance) => {
        nativeTheme.themeSource = appearance;
      }, appearance);
      await expect(page.locator("html")).toHaveAttribute("data-appearance", appearance);
      const home = {
        files: [
          {
            id: "fixture1",
            name: "juniper.vellum",
            folder: "~/code/juniper/design",
            kind: "file",
            created: 1791710400000,
            opened: 1791715380000,
            branch: "feat/onboarding",
            running: false,
          },
        ],
        folders: [{ id: "folder1", label: "~/code/juniper" }],
      };
      // TEST DATA crosses the same native state-event boundary. No fake scene or document bytes are injected.
      await app.evaluate(
        ({ BrowserWindow }, { screen, appearance, home }) => {
          const window = BrowserWindow.getAllWindows()[0];
          if (!window) throw Error("missing window");
          window.webContents.send("vellum:desktop:event", {
            type: "state",
            state: {
              folderName: "",
              active: null,
              tabs: [],
              appearance,
              platform: "mac",
              daemon: true,
              problem: "document-runtime-unavailable",
              page: screen === "A04" ? "agents" : screen.startsWith("A05") ? "signin" : "home",
              account:
                screen === "A05-device"
                  ? {
                      kind: "waiting",
                      userCode: "JPNR–8K4Q",
                      verificationUrl: "https://account.example/device?user_code=JPNR-8K4Q",
                    }
                  : { kind: "idle" },
              welcome: screen === "A01",
              home: screen === "A01" ? { files: [], folders: [] } : home,
              recovery:
                screen === "A06"
                  ? { id: "recovery1", name: "juniper.vellum", savedAt: 1791729120000 }
                  : null,
            },
          });
        },
        { screen, appearance, home },
      );
      await expect(page.locator("html")).toHaveAttribute("data-appearance", appearance);
      const { compareDesktop } = await import("./desktop-goldens");
      await compareDesktop(page, screen, appearance);
    });

test("@electron Home sort, multi-select, remove recent and disk-backed drop", async () => {
  const first = await app.firstWindow();
  await dialog([join(directory, "one")]);
  await first.getByRole("button", { name: "Open a folder", exact: true }).click();
  await expect.poll(() => app.windows().some((p) => p !== first)).toBe(true);
  const page = app.windows().find((p) => p !== first);
  if (!page) throw Error("missing window");
  await dialog([join(directory, "one", "b.vellum"), join(directory, "one", "a.vellum")]);
  await menu("file.open");
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toBeVisible();
  await menu("home");
  await page.getByRole("combobox", { name: "Sort", exact: true }).selectOption("Name");
  expect(await page.locator(".file-open strong").allTextContents()).toEqual([
    "a.vellum",
    "b.vellum",
  ]);
  await page.getByRole("combobox", { name: "View", exact: true }).selectOption("List");
  const modifier = process.platform === "darwin" ? "Meta" : "Control";
  await page
    .locator(".file-open")
    .filter({ hasText: "a.vellum" })
    .click({ modifiers: [modifier] });
  await page
    .locator(".file-open")
    .filter({ hasText: "b.vellum" })
    .click({ modifiers: [modifier] });
  await expect(page.getByText("2 selected", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Remove from recents", exact: true }).click();
  await expect(page.locator(".file-open")).toHaveCount(0);
  expect(existsSync(join(directory, "one", "a.vellum"))).toBe(true);
  await page.evaluate(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.id = "test-native-drop";
    document.body.append(input);
  });
  await page.locator("#test-native-drop").setInputFiles(join(directory, "one", "c.vellum"));
  await page.locator("#test-native-drop").evaluate((element: HTMLInputElement) => {
    const file = element.files?.[0];
    if (!file) throw Error("missing disk-backed File");
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(file);
    window.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }));
    element.remove();
  });
  await expect(page.getByRole("tab", { name: /c.vellum/ })).toBeVisible();
});

test("@electron macOS integrated title bar keeps native controls, tabs and header actions usable", async () => {
  test.skip(process.platform !== "darwin", "macOS native title-bar lane");
  const initial = await app.firstWindow();
  await expect(initial.getByRole("button", { name: "Open a folder", exact: true })).toBeVisible();
  const nativeChrome = () =>
    app.evaluate(({ BrowserWindow }) => {
      const window = BrowserWindow.getAllWindows()[0];
      if (!window) throw Error("missing native window");
      return {
        outer: window.getBounds(),
        content: window.getContentBounds(),
        controls: window.getWindowButtonPosition(),
      };
    });
  const home = await nativeChrome();
  expect(home.content.height).toBe(home.outer.height);
  expect(home.controls).toEqual({ x: 20, y: 20 });
  await expect(initial.locator(".home-appbar")).toHaveCSS("-webkit-app-region", "drag");
  await expect(initial.getByRole("button", { name: "Agents", exact: true })).toHaveCSS(
    "-webkit-app-region",
    "no-drag",
  );
  await initial.getByRole("button", { name: "Agents", exact: true }).click();
  await expect(initial.getByRole("heading", { name: "Agents and daemon" })).toBeVisible();
  await initial.getByRole("button", { name: "Home", exact: true }).click();
  await dialog([join(directory, "one")]);
  await initial.getByRole("button", { name: "Open a folder", exact: true }).click();
  await expect.poll(() => app.windows().some((p) => p !== initial)).toBe(true);
  const page = app.windows().find((p) => p !== initial);
  if (!page) throw Error("missing editor window");
  await dialog([join(directory, "one", "a.vellum"), join(directory, "one", "b.vellum")]);
  await menu("file.open");
  await expect(page.getByRole("tab", { name: /b.vellum/ })).toBeVisible();
  const editor = await nativeChrome();
  expect(editor.content.height).toBe(editor.outer.height);
  expect(editor.controls).toEqual({ x: 20, y: 20 });
  await expect(page.locator(".desktop-titlebar-drag")).toHaveCSS("-webkit-app-region", "drag");
  await expect(page.locator(".desktop-docbar")).toHaveCSS("-webkit-app-region", "drag");
  await expect(page.locator(".desktop-tabs")).toHaveCSS("-webkit-app-region", "no-drag");
  await expect(page.locator(".vc-topright")).toHaveCSS("-webkit-app-region", "no-drag");
  const first = page.getByRole("tab", { name: /a.vellum/ });
  const bounds = await first.boundingBox();
  expect(bounds?.x).toBeGreaterThanOrEqual(88);
  await first.click();
  await expect(first).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: "Close a.vellum", exact: true }).click();
  await expect(first).toHaveCount(0);
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  await expect(page.getByRole("menu")).toBeVisible();
});

test("@electron account device flow, browser approval, cancellation and offline sign-out", async () => {
  test.skip(
    process.platform === "win32",
    "POSIX executable fixture; Windows uses its native CLI lane",
  );
  // TEST DATA: copy the built app and substitute only its bundled CLI; never touch a real keychain.
  await app.close();
  const bundle = join(directory, "account fixture");
  await cp(resolve("dist"), bundle, { recursive: true });
  const binary = join(bundle, "bin", "vellum");
  const approved = join(directory, "approved");
  await writeFile(
    binary,
    `#!${process.execPath}\n
    const fs = require('node:fs');
    const command = process.argv[2];
    const send = (record) => console.log(JSON.stringify({ok:true,command,...record}));
    if (command === 'login') {
      send({state:'waiting',user_code:'ABCD-2345',verification_url:'https://account.example/device?user_code=ABCD-2345',expires_in:600});
      const timer = setInterval(() => {
        if (fs.existsSync(${JSON.stringify(approved)})) { clearInterval(timer); send({signed_in:true}); }
      }, 30);
    } else if (command === 'logout') send({signed_in:false,remote_revoked:false});
    else if (command === 'whoami') send({user:{id:'fixture',email:'fixture@example.test',name:'Fixture'}});
    else process.exit(1);
  `,
  );
  await chmod(binary, 0o700);
  app = await _electron.launch({
    args: [join(bundle, "main/index.cjs"), "--user-data", join(directory, "account-profile")],
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
  const page = await app.firstWindow();
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.getByRole("button", { name: "Continue in browser" }).click();
  await expect(page.locator(".signin-page").getByRole("status")).toHaveText(
    "◇ Waiting for approval",
  );
  await expect(page.getByText("ABCD-2345", { exact: true })).toBeVisible();
  await app.evaluate(({ shell }) => {
    (globalThis as unknown as { accountUrls: string[] }).accountUrls = [];
    shell.openExternal = async (url) => {
      (globalThis as unknown as { accountUrls: string[] }).accountUrls.push(url);
    };
  });
  await page.getByRole("button", { name: "Open approval page" }).click();
  expect(
    await app.evaluate(() => (globalThis as unknown as { accountUrls: string[] }).accountUrls),
  ).toEqual(["https://account.example/device?user_code=ABCD-2345"]);
  await page.getByRole("button", { name: "Cancel sign-in" }).click();
  await expect(page.getByRole("button", { name: "Continue in browser" })).toBeVisible();
  await page.getByRole("button", { name: "Continue in browser" }).click();
  await expect(page.getByText("ABCD-2345", { exact: true })).toBeVisible();
  await writeFile(approved, "approved");
  await expect(page.locator(".signin-page").getByRole("status")).toHaveText("✓ Signed in");
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(page.locator(".signin-page").getByRole("status")).toHaveText(
    "✓ Signed out on this computer; remote session revocation unconfirmed",
  );
  await page.getByRole("button", { name: "Check account" }).click();
  await expect(page.locator(".signin-page").getByRole("status")).toHaveText("✓ Signed in");
});

test("@electron share dialog reads daemon dependencies and closes without starting an upload", async () => {
  const initial = await app.firstWindow();
  await dialog([join(directory, "one")]);
  await initial.getByRole("button", { name: "Open a folder", exact: true }).click();
  await expect.poll(() => app.windows().length).toBe(1);
  const page = app.windows()[0];
  if (!page) throw Error("Missing folder window");
  await dialog([join(directory, "one", "a.vellum")]);
  await menu("file.open");
  await expect(page.locator("[data-content]").first()).toHaveAttribute("data-content", "ready", {
    timeout: 30_000,
  });
  await page.getByRole("button", { name: "Share", exact: true }).click();
  const shareDialog = page.getByRole("dialog", { name: "Share a.vellum" });
  await expect(shareDialog).toBeVisible();
  await expect(shareDialog.getByRole("button", { name: "Create share link" })).toBeVisible();
  await shareDialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(shareDialog).toHaveCount(0);
  const state = await page.evaluate(() =>
    window.vellumDesktop.request({ type: "share", tab: "ungranted", request: { action: "check" } }),
  );
  expect(state.ok).toBe(false);
});

// @network: local fixture API only, no installed protocol, real keychain or vendor account.
test("@electron shared snapshot protocol saves a new copy and opens it without signing in", async () => {
  test.skip(!process.env.VELLUM_TEST_SHARE_ORIGIN, "Needs the local share fixture API");
  await app.firstWindow();
  const parent = join(directory, "received");
  await mkdir(parent);
  await dialog([parent]);
  const origin = process.env.VELLUM_TEST_SHARE_ORIGIN as string;
  await app.evaluate(
    ({ app }, url) => {
      app.emit("open-url", { preventDefault() {} }, url);
    },
    `vellum://share/test_00000000000000000000000000000001?origin=${encodeURIComponent(origin)}`,
  );
  await expect
    .poll(async () => {
      for (const window of app.windows())
        if (
          !window.isClosed() &&
          (await window
            .getByRole("tab", { name: /app.vellum/ })
            .count()
            .catch(() => 0))
        )
          return true;
      return false;
    })
    .toBe(true);
  const { readdir } = await import("node:fs/promises");
  const roots = await readdir(parent);
  expect(roots).toHaveLength(1);
  const original = JSON.parse(
    await readFile(resolve("../../target/share-acceptance.json"), "utf8"),
  ) as { entry: string; files: { path: string; data: string }[] };
  const file = original.files.find((f) => f.path === original.entry);
  if (!file) throw Error("Missing fixture entry");
  expect(await readFile(join(parent, roots[0] as string, ...original.entry.split("/")))).toEqual(
    Buffer.from(file.data, "base64"),
  );
});
