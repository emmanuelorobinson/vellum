import { randomUUID } from "node:crypto";
import { required } from "./config";
import { createApi } from "./index";
import { services } from "./services";
import { diskWorkspace } from "./workspace";

const ports = services(process.env);
const workspace = diskWorkspace(required(process.env, "VELLUM_WORKER_ROOT"));
const app = createApi({ ...ports, newId: randomUUID, authorizeInput: workspace.authorize });
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => {
    void ports.close().finally(() => process.exit(0));
  });
// Bun consumes this fetch object. Only this explicitly launched cloud service opens a listener.
export default {
  port: Number(process.env.PORT ?? 3001),
  hostname: process.env.HOST ?? "127.0.0.1",
  fetch: app.fetch,
};
