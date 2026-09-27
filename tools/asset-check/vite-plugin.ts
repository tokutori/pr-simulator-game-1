import { readFile, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";
import { parse } from "smol-toml";
import ts from "typescript";
import type { Plugin } from "vite";
import { checkAssets, parseManifest } from "./index.js";

function srcsetUrls(value: string): string[] {
  const urls: string[] = [];
  let position = 0;
  while (position < value.length) {
    while (position < value.length && /[\s,]/.test(value[position] ?? "")) position++;
    if (position === value.length) break;
    const start = position;
    while (position < value.length && !/\s/.test(value[position] ?? "")) position++;
    const token = value.slice(start, position);
    const url = token.replace(/,+$/, "");
    if (url.length > 0) urls.push(url);
    if (token.endsWith(",")) continue;
    while (position < value.length && value[position] !== ",") position++;
    if (position < value.length) position++;
  }
  return urls;
}

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
  async function verifyReference(reference: string, importer: string, requireManifest = false): Promise<void> {
    if (/^data:/i.test(reference)) throw new Error(`Unregistered build asset: ${reference.slice(0, 32)}`);
    const clean = reference.split(/[?#]/, 1)[0];
    if (clean === undefined || (!requireManifest && !isAssetReference(reference))) return;
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
        for (const match of html.matchAll(/\b(src|href|poster|srcset|imagesrcset)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi)) {
          const attribute = match[1]?.toLowerCase();
          const value = match[2] ?? match[3] ?? match[4];
          if (value !== undefined) {
            if (attribute === "srcset" || attribute === "imagesrcset") {
              for (const candidate of srcsetUrls(value)) await verifyReference(candidate, importer);
            } else {
              await verifyReference(value, importer);
            }
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
            const parent = node.parent;
            const workerPath = first.text.split(/[?#]/, 1)[0] ?? first.text;
            const isWorkerSource = /\.[cm]?[jt]sx?$/i.test(workerPath);
            const isWorkerUrl = isWorkerSource && ts.isNewExpression(parent) && parent.arguments?.[0] === node &&
              ((ts.isIdentifier(parent.expression) && ["Worker", "SharedWorker"].includes(parent.expression.text)) ||
                (ts.isPropertyAccessExpression(parent.expression) && ["Worker", "SharedWorker"].includes(parent.expression.name.text)));
            if (!isWorkerUrl) references.push(first.text);
          }
          ts.forEachChild(node, visit);
        }
        visit(source);
        for (const reference of references) await verifyReference(reference, id.split("?")[0] ?? id, true);
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
