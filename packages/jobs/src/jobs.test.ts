// TEST DATA: trusted staging identifiers, fake process and queue boundaries.
import { describe, expect, it, vi } from "vitest";
import { MemoryJobQueue, renderArguments, runRenderJob, validateJob } from "./index";

const job = { id: "job-1", userId: "user-1", inputId: "bundle-1", kind: "thumbnail" as const };
describe("cloud jobs", () => {
  it("rejects paths, credentials, unsupported formats and extra payload fields", () => {
    expect(validateJob(job)).toEqual(job);
    for (const bad of [
      { ...job, inputId: "../private" },
      { ...job, token: "secret" },
      { ...job, kind: "shell" },
      { ...job, kind: "export", format: "js" },
    ]) {
      expect(() => validateJob(bad)).toThrow();
    }
  });
  it("implements ownership and idempotency in the fake queue", async () => {
    const queue = new MemoryJobQueue();
    await queue.enqueue(job);
    await queue.enqueue(job);
    await queue.enqueue({ kind: job.kind, inputId: job.inputId, userId: job.userId, id: job.id });
    expect(await queue.get(job.id, "other")).toBeNull();
    expect(await queue.get(job.id, job.userId)).toMatchObject({ id: job.id, state: "waiting" });
    await expect(queue.enqueue({ ...job, userId: "other" })).rejects.toThrow();
  });
  it("runs fixed binary arguments, cleans up on failure and hides process errors", async () => {
    expect(renderArguments(job, "/stage/input.vellum", "/stage/output")).toEqual([
      "export",
      "/stage/input.vellum",
      "--format",
      "png",
      "--scale",
      "1",
      "-o",
      "/stage/output",
    ]);
    const cleanup = vi.fn();
    const process = vi.fn().mockRejectedValue(new Error("private credential"));
    await expect(
      runRenderJob(
        job,
        {
          prepare: async () => ({
            input: "/stage/input.vellum",
            output: "/stage/output",
            cwd: "/stage",
            cleanup,
            publish: async () => "artifact-1",
          }),
        },
        process,
      ),
    ).rejects.toThrow("Render failed");
    expect(cleanup).toHaveBeenCalledOnce();
  });
});
