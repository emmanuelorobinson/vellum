import type { DesktopBridge } from "@vellum/host-electron/bridge";

declare global {
  interface Window {
    vellumDesktop: DesktopBridge;
  }
}
