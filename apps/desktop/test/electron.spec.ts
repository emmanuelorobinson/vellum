// TEST DATA. Tagged electron lane: real native binary, GUI session and built desktop bundle required.

import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
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
  await expect(page.getByText("one", { exact: true })).toBeVisible();
  await dialog([join(directory, "one", "a.vellum"), join(directory, "one", "b.vellum")]);
  await menu("file.open");
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toBeVisible();
  await expect(page.getByRole("tab", { name: /b.vellum/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.getByRole("tab", { name: /a.vellum/ }).click();
  await expect(page.getByRole("tab", { name: /a.vellum/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(
    page.getByRole("status").filter({ hasText: "Document engine unavailable" }),
  ).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => window.vellumDesktop.request({ type: "daemon" })))
    .toEqual({ ok: true, value: true });
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
