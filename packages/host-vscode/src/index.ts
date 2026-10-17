import { createHost, type HostServices } from "@vellum/host-api";
export const PACKAGE_NAME = "@vellum/host-vscode";
/** The extension owns the service implementation and its postMessage transport. */
export const createVscodeHost = (services: HostServices) => createHost("vscode", services);
