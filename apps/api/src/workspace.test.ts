// TEST DATA. Real temporary files; no Redis, Postgres or network.
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { diskWorkspace } from "./workspace";

it("isolates jobs, enforces staged ownership, refuses symlinks and publishes only output", async () => {
  const root = await mkdtemp(join(tmpdir(), "vellum-cloud-"));
  try {
    await mkdir(join(root, "inputs", "bundle"), { recursive: true });
    await writeFile(
      join(root, "inputs", "bundle", "owner.json"),
      JSON.stringify({ userId: "person" }),
    );
    await writeFile(join(root, "inputs", "bundle", "input.vellum"), "{}");
    const work = diskWorkspace(root),
      job = { id: "job", inputId: "bundle", userId: "person", kind: "thumbnail" as const };
    expect(await work.authorize("other", "bundle")).toBe(false);
    expect(await work.authorize("person", "../bundle")).toBe(false);
    const stage = await work.prepare(job);
    await writeFile(stage.output, "pixels");
    expect(await stage.publish()).toBe("job");
    expect(await readFile(join(root, "artifacts", "person", "job", "render.png"), "utf8")).toBe(
      "pixels",
    );
    await stage.cleanup();
    await symlink("/etc/hosts", join(root, "inputs", "bundle", "escape"));
    await expect(work.prepare({ ...job, id: "second" })).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
