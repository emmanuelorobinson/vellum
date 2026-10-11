import { describe, expect, it } from "vitest";
import type { ExtensionContext } from "vscode";
import { activate, deactivate } from "./extension";

describe("@vellum/extension", () => {
  it("activates and deactivates without registering anything yet", () => {
    const context = { subscriptions: [] } as unknown as ExtensionContext;
    activate(context);
    expect(context.subscriptions).toHaveLength(0);
    expect(deactivate()).toBeUndefined();
  });
});
