import { readFile, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import { parse } from "smol-toml";
import ts from "typescript";
import type { Plugin } from "vite";
import { checkAssets, parseManifest } from "./index.js";

async function fileIdentity(path: string): Promise<string> {
  const info = await stat(path, { bigint: true });
  if (!info.isFile() || info.ino === 0n) throw new Error(`Cannot identify asset file: ${path}`);
  return `${info.dev.toString()}:${info.ino.toString()}`;
}

/** Validate the resolved build graph, including resources Vite later inlines. */
export function verifiedAssets(root: string): Plugin {
  const rootPath = resolve(root);
  let registered = new Set<string>();
  let isViteAsset: (file: string) => boolean;
  function isAssetReference(reference: string): boolean {
    const clean = reference.split(/[?#]/, 1)[0];
    return clean !== undefined && (isViteAsset(clean) || /(?:\?|&)(?:raw|url)(?:&|$)/.test(reference));
  }
  async function verifyReference(reference: string, importer: string): Promise<void> {
    if (/^data:/i.test(reference)) throw new Error(`Unregistered build asset: ${reference.slice(0, 32)}`);
    const clean = reference.split(/[?#]/, 1)[0];
    if (clean === undefined || !isAssetReference(reference)) return;
    if (/^https?:\/\//i.test(reference)) throw new Error(`Unregistered build asset: ${reference}`);
    const file = clean.startsWith("/") ? join(rootPath, "web", clean.slice(1)) : resolve(dirname(importer), clean);
    if (!registered.has(await fileIdentity(file))) throw new Error(`Unregistered build asset: ${reference}`);
  }
  return {
    name: "verified-assets",
    enforce: "pre",
    apply: "build",
    configResolved(config) {
      isViteAsset = config.assetsInclude;
    },
    async buildStart() {
      await checkAssets(rootPath);
      const manifest = parseManifest(parse(await readFile(join(rootPath, "assets/manifest.toml"), "utf8")));
      registered = new Set(await Promise.all(manifest.map((asset) => fileIdentity(join(rootPath, asset.path)))));
    },
    transformIndexHtml: {
      order: "pre",
      async handler(html, context) {
        const importer = context.filename;
        if (/image-set\s*\(/i.test(html)) throw new Error(`Unsupported CSS asset syntax: ${importer}`);
        for (const match of html.matchAll(/\b(?:src|href|poster|srcset|imagesrcset)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi)) {
          const value = match[1] ?? match[2] ?? match[3];
          if (value !== undefined) {
            for (const candidate of value.split(/\s*,\s*/)) await verifyReference(candidate.trim().split(/\s+/)[0] ?? "", importer);
          }
        }
        for (const match of html.matchAll(/url\(\s*(["']?)(.*?)\1\s*\)/gi)) {
          if (match[2] !== undefined) await verifyReference(match[2], importer);
        }
      }
    },
    async transform(code, id) {
      if (/\.css(?:\?|$)/i.test(id)) {
        if (/image-set\s*\(/i.test(code)) throw new Error(`Unsupported CSS asset syntax: ${id}`);
        for (const match of code.matchAll(/url\(\s*(["']?)(.*?)\1\s*\)/gi)) {
          if (match[2] !== undefined) await verifyReference(match[2], id.split("?")[0] ?? id);
        }
      } else if (/\.[cm]?[jt]sx?(?:\?|$)/i.test(id) && !id.includes(`${sep}node_modules${sep}`) && !id.includes("/node_modules/")) {
        const source = ts.createSourceFile(id, code, ts.ScriptTarget.Latest, true);
        const references: string[] = [];
        function visit(node: ts.Node): void {
          if (ts.isNewExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "URL" &&
              node.arguments?.[1]?.getText(source).replaceAll(/\s/g, "") === "import.meta.url") {
            const first = node.arguments[0];
            if (first === undefined || !(ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))) {
              throw new Error(`Unsupported dynamic asset URL: ${id}`);
            }
            references.push(first.text);
          }
          ts.forEachChild(node, visit);
        }
        visit(source);
        for (const reference of references) await verifyReference(reference, id.split("?")[0] ?? id);
      }
    },
    async generateBundle() {
      for (const id of this.getModuleIds()) {
        const file = id.split("?")[0];
        if (file === undefined || !isAbsolute(file) || file.includes(`${sep}node_modules${sep}`)) continue;
        const query = id.slice(file.length);
        if (!isAssetReference(file + query)) continue;
        if (!registered.has(await fileIdentity(file))) {
          throw new Error(`Unregistered build asset: ${id}`);
        }
      }
    }
  };
}
