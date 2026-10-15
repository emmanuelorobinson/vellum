// Collect original native library notices for release packaging; refuses absent sources.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const [sourceArg, outArg] = process.argv.slice(2);
if (!sourceArg || !outArg)
  throw new Error("Usage: node scripts/skia-notices.mjs <synchronized Skia source> <output>");
const source = resolve(sourceArg),
  out = resolve(outArg);
mkdirSync(out, { recursive: true });
for (const [name, path] of [
  ["Skia.txt", "LICENSE"],
  ["FreeType.txt", "third_party/externals/freetype/docs/FTL.TXT"],
  ["libpng.txt", "third_party/externals/libpng/LICENSE"],
  ["zlib.txt", "third_party/externals/zlib/LICENSE"],
  ["libjpeg.txt", "third_party/externals/libjpeg-turbo/LICENSE.md"],
  ["Wuffs.txt", "third_party/externals/wuffs/release/c/wuffs-v0.3.c"],
]) {
  if (!existsSync(join(source, path))) throw new Error(`Missing notice source: ${path}`);
  copyFileSync(join(source, path), join(out, name));
}
// The font licences travel beside the native-library notices.
for (const name of ["OFL.txt", "NotoColorEmoji.OFL.txt"])
  copyFileSync(
    new URL(`../crates/adapters/text-parley/fonts/${name}`, import.meta.url),
    join(out, name),
  );
