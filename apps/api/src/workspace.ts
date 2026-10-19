import {
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  realpath,
  rename,
  rm,
} from "node:fs/promises";
import { isAbsolute, join, resolve } from "node:path";
import { identifier, validateJob, type Workspace } from "@vellum/jobs";
/** Private, operator-owned staging mount. Upload ingestion is supplied by Prompt 62, never by this API. */
export function diskWorkspace(
  root: string,
): Workspace & { authorize(userId: string, inputId: string): Promise<boolean> } {
  if (!isAbsolute(root)) throw new Error("Absolute staging root required");
  const base = resolve(root);
  const inspect = async (path: string) => {
    let bytes = 0,
      files = 0;
    async function walk(path: string, depth: number): Promise<void> {
      if (depth > 32) throw new Error("Input too deep");
      const info = await lstat(path);
      if (info.isSymbolicLink() || (!info.isFile() && !info.isDirectory()))
        throw new Error("Unsupported staged input");
      bytes += info.size;
      files++;
      if (bytes > 256 * 1024 * 1024 || files > 10000) throw new Error("Input too large");
      if (info.isDirectory())
        for (const name of await readdir(path)) await walk(join(path, name), depth + 1);
    }
    await walk(path, 0);
  };
  const authorize = async (userId: string, inputId: string) => {
    if (!identifier(userId) || !identifier(inputId)) return false;
    try {
      for (const path of [base, join(base, "inputs"), join(base, "inputs", inputId)]) {
        const info = await lstat(path);
        if (!info.isDirectory() || info.isSymbolicLink()) return false;
      }
      const manifest = join(base, "inputs", inputId, "owner.json");
      const info = await lstat(manifest);
      if (!info.isFile() || info.isSymbolicLink() || info.size > 4096) return false;
      const owner: unknown = JSON.parse(await readFile(manifest, "utf8"));
      return (
        typeof owner === "object" && owner !== null && "userId" in owner && owner.userId === userId
      );
    } catch {
      return false;
    }
  };
  return {
    authorize,
    async prepare(input) {
      const job = validateJob(input);
      if (!(await authorize(job.userId, job.inputId))) throw new Error("Input unavailable");
      const source = join(base, "inputs", job.inputId);
      await inspect(source);
      const runs = join(base, "runs");
      await mkdir(runs, { recursive: true, mode: 0o700 });
      const stage = await realpath(await mkdtemp(join(runs, "render-")));
      try {
        await cp(source, join(stage, "source"), { recursive: true, dereference: false });
        await inspect(join(stage, "source"));
        const cwd = job.kind === "pr_preview" ? join(stage, "source", "repo") : stage;
        const outputDirectory = join(stage, "output");
        const output =
          job.kind === "pr_preview" ? outputDirectory : join(outputDirectory, "render");
        if (job.kind !== "pr_preview") await mkdir(outputDirectory);
        const target = join(base, "artifacts", job.userId, job.id);
        return {
          input: join(stage, "source", "input.vellum"),
          output,
          cwd,
          async publish() {
            await inspect(outputDirectory);
            if (job.kind !== "pr_preview") {
              const bytes = await readFile(output);
              const extension =
                bytes[0] === 0x50 && bytes[1] === 0x4b
                  ? "zip"
                  : job.kind === "thumbnail"
                    ? "png"
                    : job.format;
              await rename(output, join(outputDirectory, `render.${extension}`));
            }
            await mkdir(join(base, "artifacts", job.userId), { recursive: true, mode: 0o700 });
            try {
              await rename(outputDirectory, target);
            } catch (error) {
              if (
                !error ||
                typeof error !== "object" ||
                !("code" in error) ||
                !["EEXIST", "ENOTEMPTY"].includes(String(error.code))
              )
                throw error;
            }
            return job.id;
          },
          cleanup: () => rm(stage, { recursive: true, force: true }),
        };
      } catch {
        await rm(stage, { recursive: true, force: true });
        throw new Error("Input unavailable");
      }
    },
  };
}
