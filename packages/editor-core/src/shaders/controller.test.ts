// TEST DATA: async resource boundary, no filesystem or renderer.
import { expect, it } from "vitest";
import { createShaderController, parseUniform, type ShaderView } from "./controller";

it("preserves typed compiler errors and ignores late reloads", async () => {
  const pending: ((v: ShaderView) => void)[] = [];
  const c = createShaderController({ inspect: () => new Promise((r) => pending.push(r)) });
  const a = c.refresh("grain.vsh");
  const b = c.refresh("grain.vsh");
  pending[1]!({ status: "error", kind: "invalid", message: "line 2: unknown value" });
  await b;
  pending[0]!({ status: "ready", uniforms: [] });
  await a;
  expect(c.view("grain.vsh")).toMatchObject({ status: "error", kind: "invalid" });
  expect(createShaderController({}).view("x").status).toBe("unavailable");
});
it("parses every uniform kind and rejects non-finite and malformed vectors", () => {
  expect(parseUniform({ name: "v", kind: "vector", size: 2 }, "1, 2")).toEqual([1, 2]);
  expect(parseUniform({ name: "b", kind: "boolean" }, true)).toBe(true);
  expect(() => parseUniform({ name: "b", kind: "boolean" }, "invalid")).toThrow();
  expect(() => parseUniform({ name: "n", kind: "number" }, "Infinity")).toThrow();
  expect(() => parseUniform({ name: "v", kind: "vector", size: 3 }, "1,2")).toThrow();
});
