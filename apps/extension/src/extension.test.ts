// TEST DATA: native VS Code boundary mock. Product lifecycle tests use the runtime port separately.
import { describe, expect, it, vi } from "vitest";

const mock = vi.hoisted(() => ({
  commands: new Map<string, () => Promise<unknown>>(),
  provider: vi.fn(),
  execute: vi.fn(),
  saveDialog: vi.fn(),
}));
vi.mock("vscode", () => ({
  window: {
    registerCustomEditorProvider: mock.provider.mockReturnValue({ dispose() {} }),
    activeColorTheme: { kind: 1 },
    onDidChangeActiveColorTheme: () => ({ dispose() {} }),
    showSaveDialog: mock.saveDialog,
    showErrorMessage: vi.fn(),
  },
  commands: {
    registerCommand: (id: string, action: () => Promise<unknown>) => {
      mock.commands.set(id, action);
      return { dispose() {} };
    },
    executeCommand: mock.execute,
  },
  workspace: { workspaceFolders: [{ uri: { fsPath: "/workspace" } }] },
  EventEmitter: class {
    event = () => ({ dispose() {} });
    fire() {}
    dispose() {}
  },
}));
vi.mock("./sidebar", () => ({
  DesignSidebar: class {
    refresh = vi.fn();
    dispose() {}
  },
}));
vi.mock("./connect-view", () => ({
  ConnectView: class {
    open = vi.fn();
    dispose() {}
  },
}));

import type { ExtensionContext } from "vscode";
import { activate, deactivate } from "./extension";

describe("extension activation", () => {
  it("registers the shared provider, existing commands and Prompt 56 entry points", () => {
    const context = {
      extensionPath: "/extension",
      globalStorageUri: { fsPath: "/storage" },
      subscriptions: [],
    } as unknown as ExtensionContext;
    activate(context);
    expect(mock.provider).toHaveBeenCalledWith("vellum.design", expect.anything(), {
      supportsMultipleEditorsPerDocument: true,
    });
    expect([...mock.commands.keys()]).toEqual([
      "vellum.newDesign",
      "vellum.toggleView",
      "vellum.openWelcome",
      "vellum.pauseAllAgents",
      "vellum.connectAgents",
      "vellum.refreshSidebar",
    ]);
    expect(context.subscriptions).toHaveLength(12);
    for (const subscription of context.subscriptions) subscription.dispose();
    expect(deactivate()).toBeUndefined();
  });
  it("cancelling new file creates and opens nothing", async () => {
    mock.saveDialog.mockResolvedValueOnce(undefined);
    await mock.commands.get("vellum.newDesign")?.();
    expect(mock.execute).not.toHaveBeenCalled();
  });
});
