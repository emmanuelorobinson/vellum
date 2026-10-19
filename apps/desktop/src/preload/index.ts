import type { DesktopBridge, DesktopEvent, DesktopRequest } from "@vellum/host-electron/bridge";
import { contextBridge, ipcRenderer } from "electron";
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
