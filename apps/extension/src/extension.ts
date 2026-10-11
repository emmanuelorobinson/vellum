import type { ExtensionContext } from "vscode";

/**
 * VS Code extension entry. The manifest's `main` names the built output (dist/extension.js); the
 * build step arrives with the extension prompts. Registers nothing yet.
 */
export function activate(_context: ExtensionContext): void {}

export function deactivate(): void {}
