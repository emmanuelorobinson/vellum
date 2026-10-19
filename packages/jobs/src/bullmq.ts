import { Queue, Worker } from "bullmq";
import { Redis } from "ioredis";
import { type JobQueue, type RenderJob, validateJob } from "./index";

function redis(url: string, worker = false) {
  const client = new Redis(url, { maxRetriesPerRequest: worker ? null : 1, connectTimeout: 5000 });
  client.on("error", () => {});
  return client;
}
export function bullQueue(connection: { url: string }): JobQueue {
  const client = redis(connection.url);
  const queue = new Queue<RenderJob>("vellum-render", { connection: client });
  queue.on("error", () => {});
  return {
    async enqueue(input) {
      const job = validateJob(input);
      try {
        await queue.add(job.kind, job, {
          jobId: job.id,
          attempts: 3,
          backoff: { type: "exponential", delay: 1000 },
          removeOnComplete: { age: 86400, count: 1000 },
          removeOnFail: { age: 86400, count: 1000 },
        });
        const persisted = await queue.getJob(job.id);
        if (!persisted || JSON.stringify(persisted.data) !== JSON.stringify(job)) throw new Error();
        return job.id;
      } catch {
        throw new Error("Queue unavailable or job exists");
      }
    },
    async get(id, userId) {
      try {
        const job = await queue.getJob(id);
        if (!job || job.data.userId !== userId) return null;
        return {
          id,
          state: await job.getState(),
          ...(typeof job.returnvalue === "string" ? { artifact: job.returnvalue } : {}),
        };
      } catch {
        throw new Error("Queue unavailable");
      }
    },
    async close() {
      try {
        await queue.close();
      } finally {
        client.disconnect();
      }
    },
  };
}
export interface WorkerHandle {
  on(event: "error", listener: () => void): void;
  waitUntilReady(): Promise<void>;
  close(): Promise<void>;
}
export function bullWorker(
  connection: { url: string },
  process: (job: RenderJob) => Promise<string>,
): WorkerHandle {
  const client = redis(connection.url, true);
  const worker = new Worker(
    "vellum-render",
    async (job) => {
      try {
        return await process(validateJob(job.data));
      } catch {
        throw new Error("Render failed");
      }
    },
    { connection: client, concurrency: 2 },
  );
  worker.on("error", () => {});
  return {
    on: (event, listener) => {
      worker.on(event, listener);
    },
    async waitUntilReady() {
      await worker.waitUntilReady();
    },
    async close() {
      try {
        await worker.close();
      } finally {
        client.disconnect();
      }
    },
  };
}
