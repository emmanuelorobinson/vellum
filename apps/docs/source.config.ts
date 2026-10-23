import { metaSchema, pageSchema } from "fumadocs-core/source/schema";
import { defineConfig, defineDocs } from "fumadocs-mdx/config";
import { remarkBrand, withBrand } from "./lib/brand.ts";

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    schema: pageSchema.transform((page) => ({
      ...page,
      title: withBrand(page.title),
      description: page.description === undefined ? undefined : withBrand(page.description),
    })),
  },
  meta: {
    schema: metaSchema.transform((meta) => ({
      ...meta,
      title: meta.title === undefined ? undefined : withBrand(meta.title),
    })),
  },
});

export default defineConfig({
  mdxOptions: { format: "md", remarkPlugins: [remarkBrand] },
});
