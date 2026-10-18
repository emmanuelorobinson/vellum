// TEST DATA. vscode lane: real extension host; no document editing is simulated in this suite.
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import * as vscode from "vscode";
export async function run() {
  const extension = vscode.extensions.getExtension("vellum.vellum");
  assert.ok(extension, "Vellum extension is installed in the test host");
  const api = (await extension.activate()) as { status: { editors: number; ready: number } };
  const commands = await vscode.commands.getCommands(true);
  for (const name of ["newDesign", "toggleView", "openWelcome", "pauseAllAgents"])
    assert.ok(commands.includes(`vellum.${name}`));
  const root = await mkdtemp(join(tmpdir(), "vellum-vscode-"));
  const uri = vscode.Uri.file(join(root, "host.lib.vellum"));
  try {
    await writeFile(uri.fsPath, '{"meta":{"formatVersion":1,"name":"TEST DATA"},"pages":[]}\n');
    await vscode.commands.executeCommand("vscode.openWith", uri, "vellum.design");
    for (let attempt = 0; attempt < 50 && api.status.ready === 0; attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(api.status.ready, 1, "the editor bundle boots under its CSP");
    await vscode.commands.executeCommand("vellum.toggleView");
    assert.equal(
      vscode.window.activeTextEditor?.document.uri.toString(),
      uri.toString(),
      "toggle opens the same document as text",
    );
    await vscode.commands.executeCommand("vellum.toggleView");
    await vscode.commands.executeCommand("workbench.action.splitEditor");
    assert.equal(
      await vscode.commands.executeCommand("vellum.pauseAllAgents"),
      true,
      "daemon acknowledges Pause All",
    );
    console.log(
      "VS Code host smoke passed: registration, library open, text toggle, split and Pause All.",
    );
    // This is deliberately not called full Prompt 55 acceptance. No real document edit can be made until the
    // compiled core's document ABI and the live EditorLink wire exist; hot exit and git-checkout Ops follow it.
  } finally {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await rm(root, { recursive: true, force: true });
  }
}
