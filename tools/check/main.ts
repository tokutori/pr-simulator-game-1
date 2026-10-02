import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { checkAssets } from "../asset-check/index.js";
import { assertRenderImportBoundary } from "../presentation-check/render-boundary.js";
import { record, nonEmpty } from "../shared/validation.js";

const count = await checkAssets(process.cwd());

const metadata = record(JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps", "--locked"], { encoding: "utf8" })) as unknown);
if (!Array.isArray(metadata.packages)) throw new Error("Invalid cargo metadata");
const allowed = new Map<string, readonly string[]>([
  ["birdman-game-core", ["libm"]],
  ["birdman-game-format", ["birdman-game-core", "serde", "serde_json", "sha2"]],
  ["birdman-game-cli", ["birdman-game-core", "birdman-game-format"]],
  ["birdman-game-wasm", ["birdman-game-core", "birdman-game-format", "serde", "serde_json", "sha2", "wasm-bindgen"]]
]);
for (const value of metadata.packages) {
  const pkg = record(value);
  const name = nonEmpty(pkg.name);
  const permitted = allowed.get(name);
  if (permitted === undefined || !Array.isArray(pkg.dependencies)) throw new Error(`Unexpected crate: ${name}`);
  for (const value of pkg.dependencies) {
    const dep = record(value);
    if (!permitted.includes(nonEmpty(dep.name))) throw new Error(`Forbidden dependency in ${name}: ${String(dep.name)}`);
  }
}
const sourcePaths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter((path) => path.startsWith("web/src/") && /\.[cm]?[jt]sx?$/.test(path));
const sourceTexts = await Promise.all(sourcePaths.map(async (path) => ({ path, text: await readFile(path, "utf8") })));
assertRenderImportBoundary(sourceTexts);
// Include untracked files during local development, but respect .gitignore.
const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { encoding: "utf8" }).split("\0").filter((path) => path.endsWith(".md"));
for (const path of paths) {
  const text = await readFile(path, "utf8");
  if (/\\[[\]()]/.test(text) || text.includes(":chatgpt-content-reference")) {
    throw new Error(`Unsupported GitHub math/reference delimiter in ${path}`);
  }
}
console.log(`Repository checks passed: ${String(count)} assets, dependency boundaries, render engine boundary, GitHub math delimiters`);
