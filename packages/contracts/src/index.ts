/** TypeScript contract suites: Renderer, Host, EngineClient. Shell only: no behaviour yet. */
export const PACKAGE_NAME = "@vellum/contracts";
export {
  daemonContract,
  engineContract,
  hostContract,
  syncContract,
  telemetryContract,
} from "./ports";
export { type RendererHarness, rendererContract } from "./renderer";
