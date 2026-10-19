import type { DesktopBridge, DesktopEvent, DesktopRequest } from "@vellum/host-electron/bridge";
import { contextBridge, ipcRenderer, webUtils } from "electron";
import { EVENT_CHANNEL, REQUEST_CHANNEL } from "../shared/bridge";

const bridge: DesktopBridge = {
  request: (request: DesktopRequest) => ipcRenderer.invoke(REQUEST_CHANNEL, request),
  subscribe(listener) {
    const receive = (_event: Electron.IpcRendererEvent, value: DesktopEvent) => listener(value);
    ipcRenderer.on(EVENT_CHANNEL, receive);
    return () => ipcRenderer.removeListener(EVENT_CHANNEL, receive);
  },
};
contextBridge.exposeInMainWorld("vellumDesktop", bridge);
// Only disk-backed Files from a native drop become paths. No renderer API accepts a native path.
window.addEventListener("dragover", (event) => event.preventDefault());
window.addEventListener("drop", (event) => {
  event.preventDefault();
  const paths = Array.from(event.dataTransfer?.files ?? [])
    .slice(0, 20)
    .map((file) => webUtils.getPathForFile(file))
    .filter((path) => path.endsWith(".vellum"));
  if (paths.length) void ipcRenderer.invoke("vellum:desktop:drop", paths);
});
