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

/** Native macOS controls sit inside the 40px editor bar, inset 8px from the canvas edge. */
export function windowChrome(
  platform: NodeJS.Platform,
): Pick<BrowserWindowConstructorOptions, "titleBarStyle" | "trafficLightPosition"> {
  return platform === "darwin"
    ? { titleBarStyle: "hidden", trafficLightPosition: { x: 20, y: 20 } }
    : {};
}
