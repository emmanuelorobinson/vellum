// TEST DATA. vscode lane: real extension host; no document editing is simulated in this suite.
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
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
    status: { editors: number; ready: number; connectionsReady: boolean };
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
    await writeFile(uri.fsPath, '{"meta":{"formatVersion":1,"name":"TEST DATA"},"pages":[]}\n');
    await command("vscode.openWith", uri, "vellum.design");
    for (let attempt = 0; attempt < 50 && api.status.ready === 0; attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(api.status.ready, 1, "the editor bundle boots under its CSP");
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
      "VS Code host smoke passed: registration, library open, text toggle, split and Pause All.",
    );
    // This is deliberately not called full Prompt 55 acceptance. No real document edit can be made until the
    // compiled core's document ABI and the live EditorLink wire exist; hot exit and git-checkout Ops follow it.
  } finally {
    await command("workbench.action.closeAllEditors");
    await rm(root, { recursive: true, force: true });
  }
}
