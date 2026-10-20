import { PortError } from "@vellum/host-api";
import { createBrowserHost } from "./index";
export interface SnapshotFrame {
  id: string;
  name: string;
  width: number;
  height: number;
  displayList: string;
  fonts: { handle: string; index: number; path: string }[];
  images: { id: string; path: string }[];
  css?: string;
  spec?: Record<string, unknown> | null;
}
export interface ViewerSnapshot {
  snapshot: {
    at: number;
    name: string;
    ownerName?: string;
    incomplete: boolean;
    missing: string[];
  };
  bundle: {
    schema: 1;
    entry: string;
    files: { path: string; data: string }[];
    missing: string[];
    frames: SnapshotFrame[];
  };
}
export function snapshotBytes(data: string) {
  const s = atob(data);
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}
/** Every URI is an exact bundle member. No fetch, filesystem discovery or daemon access. */
export function snapshotHost(
  bundle: ViewerSnapshot["bundle"],
  copy: (text: string) => Promise<void>,
) {
  const files = new Map(bundle.files.map((f) => [f.path, f.data]));
  const unsupported = async () => {
    throw new PortError("unsupported");
  };
  return createBrowserHost({
    readFile: async (uri) => {
      const bytes = files.get(uri);
      if (bytes === undefined) throw new PortError("missing");
      return snapshotBytes(bytes);
    },
    writeFile: async () => {
      throw new PortError("read-only");
    },
    watch: () => ({ dispose() {} }),
    discoverDaemon: async () => null,
    openExternal: unsupported,
    clipboard: {
      read: unsupported,
      write: async (data) => {
        const text = data.items["text/plain"];
        if (!text) throw new PortError("unsupported");
        await copy(new TextDecoder().decode(text));
      },
    },
    theme: () => ({ appearance: "light" }),
    onThemeChange: () => ({ dispose() {} }),
  });
}
