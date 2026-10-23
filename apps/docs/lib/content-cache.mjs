/**
 * Fumadocs compiles the remark plugin and schema transforms outside Next's module graph. Their inputs go into the
 * loader options, so cached Markdown and navigation cannot keep an old brand.
 */
export function withContentRevision(config, revision) {
  const reviseLoader = (entry) =>
    typeof entry === "object" && entry.loader?.startsWith("fumadocs-mdx/")
      ? { ...entry, options: { ...entry.options, contentRevision: revision } }
      : entry;
  const reviseRule = (rule) => {
    if (!rule || typeof rule !== "object") return rule;
    return {
      ...rule,
      ...(rule.loaders && { loaders: rule.loaders.map(reviseLoader) }),
      ...(rule.use && {
        use: Array.isArray(rule.use) ? rule.use.map(reviseLoader) : reviseLoader(rule.use),
      }),
      ...(rule.oneOf && { oneOf: rule.oneOf.map(reviseRule) }),
      ...(rule.rules && { rules: rule.rules.map(reviseRule) }),
    };
  };
  return {
    ...config,
    turbopack: {
      ...config.turbopack,
      rules: Object.fromEntries(
        Object.entries(config.turbopack?.rules ?? {}).map(([glob, rule]) => [
          glob,
          reviseRule(rule),
        ]),
      ),
    },
    webpack(input, options) {
      const result = config.webpack ? config.webpack(input, options) : input;
      result.module.rules = result.module.rules.map(reviseRule);
      return result;
    },
  };
}
