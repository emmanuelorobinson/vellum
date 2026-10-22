// Dependency rules for packages/ and apps/ (guide Prompt 2; ARCHITECTURE §2.1). scripts/check-deps.sh runs
// dependency-cruiser with this file. Dependencies point inward: the core packages never reach the UI or the
// vendors, and only the wrapper packages touch a vendor SDK.
/** @type {import("dependency-cruiser").IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "i18n-through-wrapper",
      severity: "error",
      from: { path: "^(apps|packages)/", pathNot: "^packages/i18n/" },
      to: { path: "node_modules/i18next/" },
    },
    {
      name: "editor-core-stays-headless",
      severity: "error",
      comment: "editor-core is a controller with no React and no renderer (ARCHITECTURE §2.1).",
      from: { path: "^packages/editor-core/" },
      to: {
        path: "node_modules/(react|react-dom|canvaskit-wasm)/|^packages/(editor-react|renderer-canvas2d|renderer-canvaskit)/",
      },
    },
    {
      name: "vendor-sdks-only-through-wrappers",
      severity: "error",
      comment:
        "Apps and packages reach auth, billing, database, queue and mail SDKs only through the wrapper packages (ARCHITECTURE §2.1).",
      from: {
        path: "^(apps|packages)/",
        pathNot: "^packages/(auth|billing|storage|mail|jobs|telemetry)/",
      },
      to: { path: "node_modules/(better-auth|@polar-sh|@aws-sdk|bullmq|ioredis|drizzle-orm|pg)/" },
    },
    {
      name: "packages-do-not-import-apps",
      severity: "error",
      comment: "Dependencies point inward: a package never depends on an app.",
      from: { path: "^packages/" },
      to: { path: "^apps/" },
    },
  ],
  options: {
    // node_modules stays in the graph as leaves, so rules can match the vendor modules they import.
    doNotFollow: { path: "node_modules" },
    exclude: { path: "(^|/)(dist|\\.next|\\.turbo)/|^packages/types-ts/src/generated/" },
    tsPreCompilationDeps: true,
  },
};
