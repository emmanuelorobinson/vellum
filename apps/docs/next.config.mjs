import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createMDX } from "fumadocs-mdx/next";
import { brand } from "../../packages/config/src/index.ts";
import { withContentRevision } from "./lib/content-cache.mjs";

// The brand transform lives in two places (source.config.ts and lib/brand.ts). Hash them with the name so a
// rename, or an edit to either, rebuilds cached Markdown and navigation instead of serving stale text.
const contentRevision = createHash("sha256")
  .update(brand.name)
  .update(readFileSync(new URL("./source.config.ts", import.meta.url)))
  .update(readFileSync(new URL("./lib/brand.ts", import.meta.url)))
  .digest("hex");

export default withContentRevision(
  await createMDX()({
    allowedDevOrigins: ["127.0.0.1"],
    transpilePackages: ["@vellum/config", "@vellum/ui-tokens"],
    poweredByHeader: false,
    agentRules: false,
  }),
  contentRevision,
);
