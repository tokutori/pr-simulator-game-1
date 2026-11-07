import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { parse } from "smol-toml";
import { checkAssets } from "../asset-check/index.js";
import { assertRenderImportBoundary } from "../presentation-check/render-boundary.js";
import { record } from "../shared/validation.js";
import { assertDependencyBoundary, assertNativeTextPatch } from "./dependency-boundary.js";
import { repositoryPaths } from "./repository-paths.js";

const count = await checkAssets(process.cwd());
const assetManifest = record(parse(await readFile("assets/manifest.toml", "utf8")));
if (!Array.isArray(assetManifest.assets)) throw new Error("Missing registered asset list");
const rustNotice = assetManifest.assets.map((asset: unknown) => record(asset))
  .find(asset => asset.path === "assets/wasm-runtime.LICENSE.html");
const lockSha256 = createHash("sha256").update(await readFile("Cargo.lock")).digest("hex");
if (rustNotice?.source_version !== `Cargo.lock SHA-256 ${lockSha256}`) {
  throw new Error("Regenerate the registered Web Rust license notice for the current Cargo.lock");
}

assertNativeTextPatch(parse(await readFile("Cargo.toml", "utf8")));

const metadata = record(JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps", "--locked"], { encoding: "utf8" })) as unknown);
if (!Array.isArray(metadata.packages)) throw new Error("Invalid cargo metadata");
assertDependencyBoundary(metadata.packages);
// Include untracked files during local development, but respect .gitignore.
const paths = repositoryPaths(
  execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { encoding: "utf8" }),
  execFileSync("git", ["ls-files", "--deleted", "-z"], { encoding: "utf8" })
);
const sourcePaths = paths.filter((path) => path.startsWith("web/src/") && /\.[cm]?[jt]sx?$/.test(path));
const sourceTexts = await Promise.all(sourcePaths.map(async (path) => ({ path, text: await readFile(path, "utf8") })));
assertRenderImportBoundary(sourceTexts);
for (const path of paths.filter((path) => path.endsWith(".md"))) {
  const text = await readFile(path, "utf8");
  if (/\\[[\]()]/.test(text) || text.includes(":chatgpt-content-reference")) {
    throw new Error(`Unsupported GitHub math/reference delimiter in ${path}`);
  }
}
console.log(`Repository checks passed: ${String(count)} assets, dependency boundaries, render engine boundary, GitHub math delimiters`);
