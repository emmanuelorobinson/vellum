export const PACKAGE_NAME = "@vellum/jobs";
export type RenderJob = { id: string; userId: string; inputId: string } & (
  | { kind: "thumbnail" }
  | { kind: "export"; format: "png" | "jpeg" | "webp" | "pdf" | "svg" }
  | { kind: "pr_preview"; base: string; head: string }
);
export type JobView = { id: string; state: string; artifact?: string };
export interface JobQueue {
  enqueue(job: RenderJob): Promise<string>;
  get(id: string, userId: string): Promise<JobView | null>;
  close(): Promise<void>;
}
export function identifier(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9_-]{1,128}$/.test(value);
}
export function validateJob(value: unknown): RenderJob {
  if (!value || typeof value !== "object") throw new Error("Invalid job");
  const j = value as Record<string, unknown>;
  const fields = [
    "id",
    "userId",
    "inputId",
    "kind",
    ...(j.kind === "export" ? ["format"] : j.kind === "pr_preview" ? ["base", "head"] : []),
  ];
  if (
    Object.keys(j).some((k) => !fields.includes(k)) ||
    ![j.id, j.userId, j.inputId].every(identifier) ||
    typeof j.kind !== "string" ||
    !["thumbnail", "export", "pr_preview"].includes(j.kind) ||
    (j.kind === "export" && !["png", "jpeg", "webp", "pdf", "svg"].includes(String(j.format))) ||
    (j.kind === "pr_preview" &&
      ![j.base, j.head].every((v) => typeof v === "string" && /^[a-f0-9]{40,64}$/.test(v)))
  )
    throw new Error("Invalid job");
  const job = value as RenderJob;
  const common = { id: job.id, userId: job.userId, inputId: job.inputId };
  if (job.kind === "thumbnail") return { ...common, kind: "thumbnail" };
  if (job.kind === "export") return { ...common, kind: "export", format: job.format };
  return { ...common, kind: "pr_preview", base: job.base, head: job.head };
}
export function renderArguments(job: RenderJob, input: string, output: string): string[] {
  if (job.kind === "pr_preview") return ["diff", `${job.base}..${job.head}`, "--png", "-o", output];
  return [
    "export",
    input,
    "--format",
    job.kind === "thumbnail" ? "png" : job.format,
    "--scale",
    "1",
    "-o",
    output,
  ];
}
export interface Workspace {
  prepare(job: RenderJob): Promise<{
    input: string;
    output: string;
    cwd: string;
    publish(): Promise<string>;
    cleanup(): Promise<void>;
  }>;
}
export async function runRenderJob(
  job: RenderJob,
  workspace: Workspace,
  run: (args: string[], cwd: string) => Promise<void>,
): Promise<string> {
  const valid = validateJob(job);
  const stage = await workspace.prepare(valid);
  try {
    await run(renderArguments(valid, stage.input, stage.output), stage.cwd);
    return await stage.publish();
  } catch {
    throw new Error("Render failed");
  } finally {
    await stage.cleanup();
  }
}
export class MemoryJobQueue implements JobQueue {
  private jobs = new Map<string, RenderJob>();
  async enqueue(input: RenderJob) {
    const job = validateJob(input),
      old = this.jobs.get(job.id);
    if (old && JSON.stringify(old) !== JSON.stringify(job)) throw new Error("Job exists");
    this.jobs.set(job.id, job);
    return job.id;
  }
  async get(id: string, userId: string) {
    return this.jobs.get(id)?.userId === userId ? { id, state: "waiting" } : null;
  }
  async close() {}
}
