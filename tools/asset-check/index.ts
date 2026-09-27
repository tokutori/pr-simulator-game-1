import { createHash } from "node:crypto";
import { readFile, readdir, lstat } from "node:fs/promises";
import { extname, join, posix } from "node:path";
import { parse } from "smol-toml";
import { nonEmpty, record } from "../shared/validation.js";

export interface Asset {
  readonly path: string;
  readonly sha256: string;
}

/** Requires provenance even for original assets and rejects paths outside assets/. */
export function parseManifest(value: unknown): Asset[] {
  const manifest = record(value);
  if (manifest.schema_version !== 1 || !Array.isArray(manifest.assets)) {
    throw new Error("Invalid asset manifest schema");
  }
  const paths = new Set<string>();
  return manifest.assets.map((item: unknown) => {
    const entry = record(item);
    const path = nonEmpty(entry.path);
    if (!path.startsWith("assets/") || path.includes("\\") || path.includes(":") ||
        path !== posix.normalize(path) || path === "assets/manifest.toml" ||
        path.endsWith("/") || path.split("/").some((part) => part.startsWith("."))) {
      throw new Error(`Unsafe asset path: ${path}`);
    }
    if (paths.has(path.toLowerCase())) throw new Error(`Duplicate asset path: ${path}`);
    paths.add(path.toLowerCase());
    const sha256 = nonEmpty(entry.sha256);
    if (!/^[0-9a-f]{64}$/.test(sha256)) throw new Error(`Invalid SHA-256: ${path}`);
    for (const key of ["source", "source_version", "license", "license_url", "attribution", "processing"]) {
      nonEmpty(entry[key]);
    }
    return { path, sha256 };
  });
}

/** Enumerates files deterministically; symbolic links must not bypass the manifest. */
export async function filesUnder(root: string, relative: string): Promise<string[]> {
  const result: string[] = [];
  const directory = await lstat(join(root, relative));
  if (directory.isSymbolicLink() || !directory.isDirectory()) {
    throw new Error(`Invalid asset directory: ${relative}`);
  }
  for (const entry of await readdir(join(root, relative), { withFileTypes: true })) {
    const path = posix.join(relative, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Symbolic link is not allowed: ${path}`);
    if (entry.isDirectory()) result.push(...await filesUnder(root, path));
    else result.push(path);
  }
  return result.sort();
}

export async function checkAssets(root: string): Promise<number> {
  const assets = parseManifest(parse(await readFile(join(root, "assets/manifest.toml"), "utf8")));
  const actual = (await filesUnder(root, "assets")).filter((path) => path !== "assets/manifest.toml");
  const registered = new Set(assets.map((asset) => asset.path));
  for (const path of actual) {
    if (!registered.has(path)) throw new Error(`Unregistered asset: ${path}`);
  }
  for (const asset of assets) {
    if (!actual.includes(asset.path)) throw new Error(`Missing asset: ${asset.path}`);
    const bytes = await readFile(join(root, asset.path));
    if (createHash("sha256").update(bytes).digest("hex") !== asset.sha256) {
      throw new Error(`Hash mismatch: ${asset.path}`);
    }
  }
  // Public assets will be copied from the verified manifest at build time in BPG-009.
  try {
    const stat = await lstat(join(root, "web/public"));
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error("Invalid web/public directory");
    if ((await filesUnder(root, "web/public")).length > 0) {
      throw new Error("Unmanaged web/public assets are prohibited");
    }
  } catch (error: unknown) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
  }
  // Vite also imports and inlines files from its source tree. Keep such files in
  // assets/ so their provenance and bytes are covered by the manifest.
  const sourceFiles = await filesUnder(root, "web/src");
  const codeExtensions = new Set([".ts", ".tsx", ".js", ".jsx", ".css", ".html"]);
  for (const path of sourceFiles) {
    if (!codeExtensions.has(extname(path).toLowerCase())) {
      throw new Error(`Unmanaged web source asset: ${path}`);
    }
  }
  return assets.length;
}
