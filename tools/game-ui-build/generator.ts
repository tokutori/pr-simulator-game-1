import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { readFile } from "node:fs/promises";
import { Ajv2020, _ } from "ajv/dist/2020.js";
import standaloneCode from "ajv/dist/standalone/index.js";
import { build } from "esbuild";
import { compile } from "json-schema-to-typescript";
import ts from "typescript";
import { numericFormats } from "./numeric-formats.js";

export const schemaEntries = [
  { key: "view", name: "GameUiView", filename: "game-ui-view", group: "serialize" },
  { key: "action", name: "GameUiAction", filename: "game-ui-action", group: "deserialize" },
  { key: "input", name: "GameUiPlatformInput", filename: "game-ui-input", group: "deserialize" },
  { key: "effects", name: "GameUiUpdate", filename: "game-ui-effects", group: "serialize" },
  { key: "frame", name: "GameUiFrame", filename: "game-ui-frame", group: "serialize" },
  { key: "tail_input", name: "TailInputDocument", filename: "tail-input", group: "deserialize" },
  { key: "tail_snapshot", name: "TailSnapshotDocument", filename: "tail-snapshot", group: "serialize" },
  { key: "tail_prepared", name: "TailPreparedLaunchDocument", filename: "tail-prepared", group: "serialize" },
  { key: "tail_scenario", name: "EnvironmentIdentity", filename: "tail-scenario", group: "serialize" },
  { key: "tail_identity", name: "FlightRecordTailIdentityDocument", filename: "tail-identity", group: "serialize" },
  { key: "tail_incidence", name: "TailIncidenceDocument", filename: "tail-incidence", group: "serialize" },
  { key: "tail_finalization", name: "TailFlightRecordFinalizationDocument", filename: "tail-finalization", group: "serialize" }
] as const;

const schemaGroups = ["serialize", "deserialize"] as const;
export type GameUiSchemas = Readonly<Record<typeof schemaGroups[number], Readonly<Record<string, unknown>>>>;
export type GeneratedFile = Readonly<{ path: string; content: string }>;

const generatedBanner = "Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate.";

export function parseSchemaBundle(text: string): GameUiSchemas {
  const document: unknown = JSON.parse(text);
  if (!isObject(document)) throw new Error("Game UI schema exporter must return an object");
  const keys = Object.keys(document);
  if (keys.length !== schemaGroups.length || keys.some((key) => !schemaGroups.some((group) => group === key))) {
    throw new Error("Game UI schema exporter returned unexpected schema roots");
  }
  const schemas = Object.fromEntries(schemaGroups.map((group) => {
    const schema = document[group];
    if (!isObject(schema) || schema["$schema"] !== "https://json-schema.org/draft/2020-12/schema" || !isObject(schema["$defs"])) {
      throw new Error(`Invalid ${group} schema group`);
    }
    for (const entry of schemaEntries.filter((entry) => entry.group === group)) {
      if (!isObject(schema["$defs"][entry.name])) throw new Error(`Invalid ${entry.name} schema root`);
    }
    return [group, schema];
  }));
  return schemas as GameUiSchemas;
}

export async function generateGameUiFiles(schemas: GameUiSchemas, root: string): Promise<readonly GeneratedFile[]> {
  const require = createRequire(import.meta.url);
  const formatsPath = resolve(root, "tools/game-ui-build/numeric-formats.ts");
  const ajv = new Ajv2020({
    strict: true,
    strictNumbers: true,
    allErrors: true,
    inlineRefs: false,
    code: { source: true, esm: true, formats: _`require(${formatsPath}).numericFormats` }
  });
  for (const [name, format] of Object.entries(numericFormats)) ajv.addFormat(name, format);
  const files: GeneratedFile[] = [];
  const validatorRoots: Record<string, string> = {};
  const sourceHashes: Record<string, string> = {};
  for (const group of schemaGroups) {
    const schema = { ...schemas[group], $id: `https://tokutori.github.io/pr-simulator-game-1/game-ui/${group}` };
    ajv.addSchema(schema);
    const canonical = `${JSON.stringify(schema, null, 2)}\n`;
    sourceHashes[group] = createHash("sha256").update(canonical).digest("hex");
    files.push({ path: `tools/game-ui-build/generated/game-ui-${group}.schema.json`, content: canonical });
  }
  for (const entry of schemaEntries) {
    const group = schemas[entry.group];
    const definitions = group["$defs"];
    if (!isObject(definitions)) throw new Error(`Invalid ${entry.group} schema group`);
    const definition = definitions[entry.name];
    if (!isObject(definition)) throw new Error(`Invalid ${entry.name} schema root`);
    validatorRoots[`validate${entry.name}`] = `https://tokutori.github.io/pr-simulator-game-1/game-ui/${entry.group}#/$defs/${entry.name}`;
    const schema: Record<string, unknown> = { ...definition, $defs: definitions, $schema: group["$schema"], title: entry.name };
    const declarations = await compile(schema, entry.name, {
      bannerComment: "",
      unknownAny: true,
      strictIndexSignatures: true,
      style: { singleQuote: false, tabWidth: 2, printWidth: 120, trailingComma: "none", semi: true }
    });
    files.push({ path: `web/src/generated/${entry.filename}.ts`, content: `/* ${generatedBanner} */\n${readonlyDeclarations(declarations)}` });
  }
  const standalone = standaloneCode(ajv, validatorRoots);
  const bundled = await build({
    stdin: { contents: standalone, resolveDir: root, sourcefile: "game-ui-validators.ts", loader: "js" },
    bundle: true,
    platform: "browser",
    format: "esm",
    target: "es2023",
    write: false,
    charset: "utf8",
    legalComments: "eof",
    banner: { js: `/* ${generatedBanner} */` }
  });
  const output = bundled.outputFiles[0];
  if (output === undefined || bundled.outputFiles.length !== 1) throw new Error("Standalone game UI validator bundling produced unexpected outputs");
  files.push({ path: "web/src/generated/game-ui-validators.js", content: normalizeText(output.text) });
  files.push({ path: "web/src/generated/game-ui-validators.d.ts", content: [
    `/* ${generatedBanner} */`,
    'import type { ValidateFunction } from "ajv";',
    ...schemaEntries.map((entry) => `import type { ${entry.name} } from "./${entry.filename}.js";`),
    ...schemaEntries.map((entry) => `export declare const validate${entry.name}: ValidateFunction<${entry.name}>;`),
    ""
  ].join("\n") });
  const notices = await Promise.all([{ name: "ajv", license: "LICENSE" }, { name: "esbuild", license: "LICENSE.md" }].map(async ({ name, license }) => {
    const packageDirectory = dirname(require.resolve(`${name}/package.json`));
    return `${name}\n${await readFile(resolve(packageDirectory, license), "utf8")}`;
  }));
  files.push({ path: "assets/game-ui-validators.LICENSE.txt", content: normalizeText(notices.join("\n\n")) });
  const generators = Object.fromEntries(await Promise.all(Object.entries({ ajv: "ajv", jsonSchemaToTypescript: "json-schema-to-typescript", esbuild: "esbuild", typescript: "typescript" })
    .map(async ([key, name]): Promise<readonly [string, string]> => {
      const metadata: unknown = JSON.parse(await readFile(require.resolve(`${name}/package.json`), "utf8"));
      if (!isObject(metadata) || typeof metadata["version"] !== "string") throw new Error(`Missing ${name} generator version`);
      return [key, metadata["version"]];
    })));
  files.push({ path: "tools/game-ui-build/generated/game-ui-generation.json", content: `${JSON.stringify({ schemaVersion: 2, generators, sources: sourceHashes }, null, 2)}\n` });
  return files;
}

export function readonlyDeclarations(source: string): string {
  const sourceFile = ts.createSourceFile("generated.ts", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const transformed = ts.transform(sourceFile, [(context) => {
    const visitor: ts.Visitor = (node) => {
      const visited = ts.visitEachChild(node, visitor, context);
      if (ts.isPropertySignature(visited)) {
        const modifiers = visited.modifiers?.filter((modifier) => modifier.kind !== ts.SyntaxKind.ReadonlyKeyword) ?? [];
        return context.factory.updatePropertySignature(visited, [context.factory.createModifier(ts.SyntaxKind.ReadonlyKeyword), ...modifiers], visited.name, visited.questionToken, visited.type);
      }
      if (ts.isIndexSignatureDeclaration(visited)) {
        const modifiers = visited.modifiers?.filter((modifier) => modifier.kind !== ts.SyntaxKind.ReadonlyKeyword) ?? [];
        return context.factory.updateIndexSignature(visited, [context.factory.createModifier(ts.SyntaxKind.ReadonlyKeyword), ...modifiers], visited.parameters, visited.type);
      }
      if (ts.isArrayTypeNode(visited) || ts.isTupleTypeNode(visited)) {
        return context.factory.createTypeOperatorNode(ts.SyntaxKind.ReadonlyKeyword, visited);
      }
      return visited;
    };
    return (node) => ts.visitEachChild(node, visitor, context);
  }]);
  try {
    const result = transformed.transformed[0];
    if (result === undefined) throw new Error("Readonly type generation produced no source file");
    return normalizeText(ts.createPrinter({ newLine: ts.NewLineKind.LineFeed }).printFile(result));
  } finally {
    transformed.dispose();
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeText(value: string): string {
  return `${value.replaceAll("\r\n", "\n").trimEnd()}\n`;
}
