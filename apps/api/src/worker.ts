import { Pool } from "@vellum/auth/postgres";
import { postgresEntitlements } from "@vellum/billing/postgres";
import { runRenderJob } from "@vellum/jobs";
import { bullWorker } from "@vellum/jobs/bullmq";
import { required } from "./config";
import { binaryRunner } from "./process";
import { diskWorkspace } from "./workspace";

const pool = new Pool({ connectionString: required(process.env, "DATABASE_URL"), max: 4 });
pool.on("error", () => {
  console.error("Render database unavailable");
});
const store = postgresEntitlements(pool);
const productId = required(process.env, "POLAR_PRO_PRODUCT_ID");
const workspace = diskWorkspace(required(process.env, "VELLUM_WORKER_ROOT"));
const run = binaryRunner(required(process.env, "VELLUM_WORKER_BINARY"));
const worker = bullWorker({ url: required(process.env, "REDIS_URL") }, async (job) => {
  const feature = job.kind === "pr_preview" ? "pr_previews" : "share_links";
  if (!(await store.grants(job.userId, Date.now(), [productId])).includes(feature))
    throw new Error("Pro required");
  return runRenderJob(job, workspace, run);
});
// Never log raw BullMQ/Redis/process exceptions, which can include credentials or input content.
worker.on("error", () => {
  console.error("Render worker unavailable");
});
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => {
    void worker
      .close()
      .then(() => pool.end())
      .finally(() => process.exit(0));
  });
