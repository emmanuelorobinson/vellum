// TEST DATA. @native: needs the native vellum binary, daemon socket permissions and temporary files.
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runRenderJob } from "@vellum/jobs";
import { expect, it } from "vitest";
import { binaryRunner } from "./process";
import { diskWorkspace } from "./workspace";

const binary = process.env.VELLUM_TEST_BINARY;
it.skipIf(!binary)(
  "@native cloud worker exports a staged document through the real vellum binary",
  async () => {
    const root = await mkdtemp(join(tmpdir(), "vellum-cloud-native-"));
    try {
      await mkdir(join(root, "inputs", "bundle"), { recursive: true });
      await writeFile(
        join(root, "inputs", "bundle", "owner.json"),
        JSON.stringify({ userId: "person" }),
      );
      await writeFile(
        join(root, "inputs", "bundle", "input.vellum"),
        JSON.stringify({
          meta: { formatVersion: 1, name: "TEST DATA: cloud", createdWith: "test" },
          pages: [
            {
              id: "page000001",
              name: "Page",
              children: [
                {
                  type: "frame",
                  id: "frame00001",
                  name: "Frame",
                  width: 32,
                  height: 24,
                  fills: [{ type: "solid", color: "#FF0000" }],
                },
              ],
            },
          ],
        }),
      );
      const id = await runRenderJob(
        { id: "render", inputId: "bundle", userId: "person", kind: "thumbnail" },
        diskWorkspace(root),
        binaryRunner(binary ?? ""),
      );
      expect(id).toBe("render");
      const directory = join(root, "artifacts", "person", id);
      const files = await readdir(directory);
      expect(files.length).toBe(1);
      const bytes = await readFile(join(directory, files[0] ?? ""));
      expect(bytes.subarray(0, 4).toString("hex")).toBe("89504e47");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
  30000,
);
it.skipIf(!binary)(
  "@native cloud worker exports PDF and renders hosted PR previews from a staged repo",
  async () => {
    const { execFileSync } = await import("node:child_process");
    const root = await mkdtemp(join(tmpdir(), "vellum-cloud-native-"));
    const bundle = join(root, "inputs", "bundle");
    const document = (color: string) =>
      JSON.stringify({
        meta: { formatVersion: 1, name: "TEST DATA: cloud", createdWith: "test" },
        pages: [
          {
            id: "page000001",
            name: "Page",
            children: [
              {
                type: "frame",
                id: "frame00001",
                name: "Frame",
                width: 32,
                height: 24,
                fills: [{ type: "solid", color }],
              },
            ],
          },
        ],
      });
    try {
      const repo = join(bundle, "repo");
      await mkdir(repo, { recursive: true });
      await writeFile(join(bundle, "owner.json"), JSON.stringify({ userId: "person" }));
      await writeFile(join(bundle, "input.vellum"), document("#FF0000"));
      const workspace = diskWorkspace(root),
        runner = binaryRunner(binary ?? "");
      await runRenderJob(
        { id: "pdf", inputId: "bundle", userId: "person", kind: "export", format: "pdf" },
        workspace,
        runner,
      );
      const pdf = await readFile(join(root, "artifacts", "person", "pdf", "render.pdf"));
      expect(pdf.subarray(0, 4).toString()).toBe("%PDF");
      const git = (args: string[]) =>
        execFileSync("git", args, {
          cwd: repo,
          encoding: "utf8",
          env: { PATH: process.env.PATH, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null" },
        }).trim();
      git(["init", "-q"]);
      await writeFile(join(repo, "design.vellum"), document("#FF0000"));
      git(["add", "."]);
      git([
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.test",
        "commit",
        "-qm",
        "before",
      ]);
      const base = git(["rev-parse", "HEAD"]);
      await writeFile(join(repo, "design.vellum"), document("#00FF00"));
      git(["add", "."]);
      git(["-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "-qm", "after"]);
      const head = git(["rev-parse", "HEAD"]);
      await runRenderJob(
        { id: "preview", inputId: "bundle", userId: "person", kind: "pr_preview", base, head },
        workspace,
        runner,
      );
      const files = await readdir(join(root, "artifacts", "person", "preview"));
      expect(files.some((x) => x.endsWith(".png"))).toBe(true);
      expect(files.some((x) => x.endsWith(".json"))).toBe(true);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
  30000,
);
