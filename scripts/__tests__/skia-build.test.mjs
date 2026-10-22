import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { buildKey, checkBindings, config } from "../skia-build.mjs";

test("Skia identities separate targets and every patch change", () => {
  assert.notEqual(buildKey("aarch64-apple-darwin"), buildKey("x86_64-apple-darwin"));
  assert.notEqual(buildKey("test", [["p", "old"]]), buildKey("test", [["p", "new"]]));
  assert.equal(buildKey("test"), buildKey("test"));
  assert.notEqual(buildKey("test", [], "SDK 1"), buildKey("test", [], "SDK 2"));
});
test("retired macOS 27 patch cannot silently return", () => {
  assert.throws(
    () =>
      checkBindings("pub type std___hash_table___node_allocator = std___rebind_alloc<_Traits>;"),
    /regression/,
  );
  checkBindings("pub type Valid = u32;");
});
test("every Cargo entry uses the same source-built FreeType policy", () => {
  const cargo = readFileSync(new URL("../../Cargo.toml", import.meta.url), "utf8");
  assert.match(cargo, /skia-safe = \{ version = "=0.153.3", default-features = false/);
  for (const f of config.features) assert.ok(cargo.includes(`"${f}"`));
  assert.ok(!config.features.includes("binary-cache") && !config.features.includes("textlayout"));
  assert.ok(
    JSON.parse(
      readFileSync(new URL("../../.cargo/config.toml", import.meta.url), "utf8").match(
        /^SKIA_GN_ARGS = (.+)$/m,
      )[1],
    ) === config.gn,
  );
});
