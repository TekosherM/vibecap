/**
 * Nitro's Vercel output does not copy PGLite's wasm/data next to the bundled
 * module. Preview (no DATABASE_URL) loads PGLite via
 * `new URL("./pglite.wasm", import.meta.url)` from `_libs/electric-sql__pglite.mjs`.
 * Production Neon never hits this path. Safe no-op if the nitro folder is absent.
 */
import { copyFileSync, existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const srcDir = join(process.cwd(), "node_modules/@electric-sql/pglite/dist");
const destDir = join(
  process.cwd(),
  ".vercel/output/functions/__server.func/_libs",
);

if (!existsSync(srcDir) || !existsSync(destDir)) {
  console.log("[pglite-assets] skip (src or nitro output missing)");
  process.exit(0);
}

for (const name of readdirSync(srcDir)) {
  if (!name.endsWith(".wasm") && !name.endsWith(".data")) continue;
  copyFileSync(join(srcDir, name), join(destDir, name));
  console.log("[pglite-assets] copied", name);
}
