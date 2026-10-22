// TEST DATA. vscode lane: real extension host, real daemon and the editor WASM engine in the webview.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import * as vscode from "vscode";

async function command(id: string, ...args: unknown[]) {
  const timeout = new Promise<never>((_, reject) => {
    const timer = setTimeout(() => reject(Error(`Timed out: ${id}`)), 20_000);
    timer.unref();
  });
  const result = await Promise.race([vscode.commands.executeCommand(id, ...args), timeout]);
  console.log(`Verified host command: ${id}`);
  return result;
}
export async function run() {
  console.log("Starting Vellum host smoke");
  const extension = vscode.extensions.getExtension("vellum.vellum");
  assert.ok(extension, "Vellum extension is installed in the test host");
  const api = (await extension.activate()) as {
    status: { editors: number; ready: number; live: number; connectionsReady: boolean };
  };
  console.log("Extension activated");
  const commands = await vscode.commands.getCommands(true);
  for (const name of [
    "newDesign",
    "toggleView",
    "openWelcome",
    "pauseAllAgents",
    "connectAgents",
    "refreshSidebar",
  ])
    assert.ok(commands.includes(`vellum.${name}`));
  const root = await mkdtemp(join(tmpdir(), "vellum-vscode-"));
  const uri = vscode.Uri.file(join(root, "host.lib.vellum"));
  try {
    await writeFile(
      uri.fsPath,
      '{"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"TEST DATA","kind":"library"},"pages":[]}\n',
    );
    await command("vscode.openWith", uri, "vellum.design");
    for (let attempt = 0; attempt < 50 && api.status.ready === 0; attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(api.status.ready, 1, "the editor bundle boots under its CSP");
    // Before Pause All below: a paused daemon refuses the agent's write this check makes.
    await liveEditing(api);
    await command("vscode.openWith", uri, "vellum.design");
    await command("vellum.toggleView");
    assert.equal(
      vscode.window.activeTextEditor?.document.uri.toString(),
      uri.toString(),
      "toggle opens the same document as text",
    );
    await command("vellum.toggleView");
    await command("workbench.action.splitEditor");
    assert.equal(await command("vellum.pauseAllAgents"), true, "daemon acknowledges Pause All");
    assert.equal(await command("vellum.connectAgents"), true);
    for (let attempt = 0; attempt < 50 && !api.status.connectionsReady; attempt++)
      await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(
      api.status.connectionsReady,
      true,
      "Connect Agents boots under the restricted CSP",
    );
    await command("workbench.action.closeActiveEditor");
    await command("vellum.recent.focus");
    await command("vellum.sessions.focus");
    await command("vellum.refreshSidebar");
    console.log(
      "VS Code host smoke passed: registration, library open, text toggle, split, Pause All and live editing.",
    );
  } finally {
    await command("workbench.action.closeAllEditors");
    await rm(root, { recursive: true, force: true });
  }
}

const wait = async (what: string, ready: () => boolean | Promise<boolean>) => {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await ready()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.fail(`Timed out: ${what}`);
};

/** The webview's engine replicates the daemon's (ADR-033): an agent's change reaches the editor, makes the
 * document dirty, and the editor's Save writes it. */
async function liveEditing(api: { status: { live: number } }) {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  assert.ok(folder, "the test opens a workspace folder");
  const file = join(folder, "live.vellum");
  await writeFile(
    file,
    JSON.stringify({
      meta: { formatVersion: 1, name: "TEST DATA", createdWith: "TEST DATA" },
      pages: [
        {
          id: "page000001",
          name: "TEST DATA",
          children: [
            {
              id: "frame00001",
              type: "frame",
              name: "Before",
              width: 100,
              height: 100,
              position: { type: "absolute", x: 0, y: 0 },
            },
          ],
        },
      ],
    }),
  );
  const uri = vscode.Uri.file(file);
  await command("vscode.openWith", uri, "vellum.design");
  await wait("the webview's engine opens its editor at the daemon", () => api.status.live >= 1);
  const binary = process.env.VELLUM_TEST_BINARY;
  const runDir = process.env.VELLUM_RUN_DIR;
  assert.ok(binary && runDir);
  const agent = spawnSync(
    binary,
    [
      "--run-dir",
      runDir,
      "tool",
      "update_nodes",
      "--document",
      file,
      "--updates",
      JSON.stringify([{ id: "frame00001", set: { name: "By an agent" } }]),
    ],
    { cwd: folder, encoding: "utf8" },
  );
  assert.equal(agent.status, 0, `the agent's write: ${agent.stderr}`);
  const tab = () =>
    vscode.window.tabGroups.all
      .flatMap((g) => g.tabs)
      .find((t) => (t.input as { uri?: vscode.Uri })?.uri?.fsPath === file);
  await wait("the agent's unsaved change makes the document dirty", () => tab()?.isDirty === true);
  assert.ok(
    !(await readFile(file, "utf8")).includes("By an agent"),
    "the editor saves, not the daemon",
  );
  await command("workbench.action.files.save");
  await wait("Save writes the engine's document", async () =>
    (await readFile(file, "utf8")).includes("By an agent"),
  );
  await wait("saved is clean", () => tab()?.isDirty === false);
  console.log("Verified live editing: agent change, dirty state and Save");
  await command("workbench.action.closeActiveEditor");
}
