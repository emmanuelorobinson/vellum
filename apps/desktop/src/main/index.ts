import type { BrowserWindowConstructorOptions } from "electron";

/**
 * Renderer security defaults every window must use. Nothing may relax them: the editor bundle is
 * untrusted web content as far as the main process is concerned.
 */
export const WEB_PREFERENCES = {
  contextIsolation: true,
  sandbox: true,
  nodeIntegration: false,
} satisfies NonNullable<BrowserWindowConstructorOptions["webPreferences"]>;
