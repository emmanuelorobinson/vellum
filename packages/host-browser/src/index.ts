import { createHost, type HostServices, PortError } from "@vellum/host-api";
export const PACKAGE_NAME = "@vellum/host-browser";
const unsupported: HostServices = {
  async readFile() {
    throw new PortError("unsupported");
  },
  async writeFile() {
    throw new PortError("read-only");
  },
  watch() {
    throw new PortError("unsupported");
  },
  async discoverDaemon() {
    return null;
  },
  async openExternal() {
    throw new PortError("unsupported");
  },
  clipboard: {
    async read() {
      throw new PortError("unsupported");
    },
    async write() {
      throw new PortError("unsupported");
    },
  },
  theme() {
    return { appearance: "light" };
  },
  onThemeChange() {
    return { dispose() {} };
  },
};
/** Browser composition supplies granted file/clipboard/link capabilities, never ambient file access. */
export const createBrowserHost = (services: HostServices = unsupported) =>
  createHost("browser", services);
