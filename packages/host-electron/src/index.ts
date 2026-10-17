import { createHost, type HostServices } from "@vellum/host-api";
export const PACKAGE_NAME = "@vellum/host-electron";
/** Only a narrow, trusted preload service is injected; the editor never imports Electron. */
export const createElectronHost = (services: HostServices) => createHost("electron", services);
