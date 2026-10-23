import { brand } from "@vellum/config";

/** What Markdown, frontmatter and meta.json write for the product's name. */
export const BRAND_TOKEN = "{{brand}}";

export const withBrand = (text: string): string => text.split(BRAND_TOKEN).join(brand.name);

type Node = { value?: unknown; children?: Node[] };

/**
 * A remark plugin: `{{brand}}` in text, inline code and code blocks becomes the product's name. The pages never
 * spell it out, so a rename is one edit in `@vellum/config`.
 */
export function remarkBrand() {
  const walk = (node: Node): void => {
    if (typeof node.value === "string") node.value = withBrand(node.value);
    for (const child of node.children ?? []) walk(child);
  };
  return (tree: Node) => walk(tree);
}
