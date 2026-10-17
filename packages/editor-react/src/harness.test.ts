import { describe, expect, it } from "vitest";
import { componentCases, componentGoldens, createHarnessServices } from "./harness";

describe("component harness", () => {
  it("tracks every component specimen × appearance with a unique story and explicit pending golden", () => {
    expect(componentCases).toHaveLength(1008);
    expect(new Set(componentCases.map((c) => c.id)).size).toBe(1008);
    for (const mode of ["light", "dark", "hc-light", "hc-dark"])
      expect(componentCases.filter((c) => c.mode === mode)).toHaveLength(252);
    expect(componentGoldens()).toEqual({ pending: 1008, ready: 0 });
    expect(
      componentCases.every(
        (c) =>
          c.box.width > 0 &&
          c.box.height > 0 &&
          c.variant &&
          (c.reference === "available"
            ? c.frameId && c.nodeId
            : c.frameId === null && c.nodeId === null),
      ),
    ).toBe(true);
    expect(componentCases.filter((c) => c.reference === "missing")).toHaveLength(303);
  });
  it("creates independent offline fakes for each story", () => {
    const a = createHarnessServices();
    const b = createHarnessServices();
    expect(a.engine).not.toBe(b.engine);
    expect(a.daemon.state).toBe("offline");
    expect(a.telemetry.enabled).toBe(false);
    a.engine.dispose();
    expect(b.engine.state).toBe("ready");
    a.dispose();
    b.dispose();
  });
});
