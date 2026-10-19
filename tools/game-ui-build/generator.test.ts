import { copyFile, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { ValidateFunction } from "ajv";
import ts from "typescript";
import { generateGameUiFiles, parseSchemaBundle, readonlyDeclarations, schemaEntries } from "./generator.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const nestedDeclaration = "export interface Nested { values: { position: number[] }[] }";

function schemaBundle(): string {
  return JSON.stringify(Object.fromEntries(["serialize", "deserialize"].map((group) => [group, {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $defs: {
      identifier: { type: "integer", format: "uint32", minimum: 0 },
      coordinates: { type: "array", minItems: 3, maxItems: 3, items: { type: "number", format: "double" } },
      GameUiStoredRecordPreview: { type: "array", maxItems: 3, items: { type: "string" } },
      GameUiStoredRecord: { type: "object", properties: { saved_at: { type: "string", maxLength: 256 } } },
      ...Object.fromEntries(schemaEntries.filter((entry) => entry.group === group).map((entry) => [entry.name, {
        type: "object",
        "x-boundary-bytes": 131072,
        additionalProperties: false,
        required: ["kind", "coordinates", ...(group === "serialize" ? ["id"] : [])],
        properties: {
          kind: { const: "fixture", type: "string" },
          id: { $ref: "#/$defs/identifier" },
          coordinates: { $ref: "#/$defs/coordinates" },
          notification: { type: "string", maxLength: 2048 }
        }
      }]))
    }
  }])));
}

describe("Rust game UI schema generation", () => {
  it("generates the same validator with ordinary and junction-backed dependencies", async () => {
    const directory = await mkdtemp(resolve(root, "target/game-ui-module-path-"));
    const ordinary = resolve(directory, "ordinary");
    const linked = resolve(directory, "linked");
    const require = createRequire(import.meta.url);
    const runtime = require.resolve("ajv/dist/runtime/ucs2length.js");
    try {
      for (const fixture of [ordinary, linked]) {
        await mkdir(resolve(fixture, "tools/game-ui-build"), { recursive: true });
        await copyFile(resolve(root, "tools/game-ui-build/numeric-formats.ts"), resolve(fixture, "tools/game-ui-build/numeric-formats.ts"));
      }
      const ordinaryRuntime = resolve(ordinary, "node_modules/ajv/dist/runtime/ucs2length.js");
      await mkdir(dirname(ordinaryRuntime), { recursive: true });
      await copyFile(runtime, ordinaryRuntime);
      await symlink(resolve(ordinary, "node_modules"), resolve(linked, "node_modules"), "junction");
      const schemas = parseSchemaBundle(schemaBundle());
      const source = (files: Awaited<ReturnType<typeof generateGameUiFiles>>) =>
        files.find((file) => file.path === "web/src/generated/game-ui-validators.js")?.content;
      const normal = source(await generateGameUiFiles(schemas, ordinary));
      expect(typeof normal).toBe("string");
      expect(source(await generateGameUiFiles(schemas, linked))).toBe(normal);
    } finally { await rm(directory, { recursive: true, force: true }); }
  });

  it("rejects mismatched schema roots and unregistered numeric formats", async () => {
    expect(() => parseSchemaBundle("{}")).toThrow("unexpected schema roots");
    expect(() => parseSchemaBundle(schemaBundle().replace("GameUiView", "OtherView"))).toThrow("Invalid GameUiView");
    await expect(generateGameUiFiles(parseSchemaBundle(schemaBundle().replaceAll('"double"', '"unknown_number_format"')), root)).rejects.toThrow("unknown format");
  });

  it("generates deeply readonly declarations and fixed-size tuples", async () => {
    const files = await generateGameUiFiles(parseSchemaBundle(schemaBundle()), root);
    const declaration = files.find((file) => file.path === "web/src/generated/game-ui-view.ts");
    expect(declaration?.content).toContain('readonly kind: "fixture"');
    expect(declaration?.content).toContain("readonly coordinates: Coordinates");
    expect(declaration?.content).toContain("type Coordinates = readonly [");
    expect(declaration?.content).not.toContain("[k: string]");
    expect(readonlyDeclarations(nestedDeclaration))
      .toMatch(/readonly values: readonly \{\s*readonly position: readonly number\[\]/);
  });

  it("checks generated readonly and tuple contracts with the TypeScript compiler", async () => {
    const files = await generateGameUiFiles(parseSchemaBundle(schemaBundle()), root);
    const declaration = files.find((file) => file.path === "web/src/generated/game-ui-view.ts");
    if (declaration === undefined) throw new Error("Missing generated view declarations");
    const imports = 'import type { Coordinates, GameUiView } from "./generated.js";\nimport type { Nested } from "./nested.js";\n'
      + `import type { GameUiEffect } from ${JSON.stringify(resolve(root, "web/src/generated/game-ui-effects.js").replace(/\\/g, "/"))};\n`;
    const bindings = "declare const view: GameUiView;\ndeclare const nested: Nested;\n";
    const fixtures = [
      { name: "positive", source: "const coordinates: Coordinates = [1, 2, 3];\nconst length: 3 = coordinates.length;\nconst position: number = nested.values[0]!.position[0]!;\nconst kind: 'fixture' = view.kind;\nconst effect: GameUiEffect = { kind: 'record', action: { kind: 'seek_analysis', time_seconds: 0 } };", codes: [] },
      { name: "non-record-ui-action", source: "const effect: GameUiEffect = { kind: 'record', action: { kind: 'toggle_technical' } };", codes: [2322] },
      { name: "property-write", source: "view.coordinates = [1, 2, 3];", codes: [2540] },
      { name: "nested-property-write", source: "nested.values[0]!.position = [1, 2, 3];", codes: [2540] },
      { name: "nested-index-write", source: "nested.values[0]!.position[0] = 1;", codes: [2542] },
      { name: "outer-array-push", source: "nested.values.push({ position: [1, 2, 3] });", codes: [2339] },
      { name: "inner-array-push", source: "nested.values[0]!.position.push(1);", codes: [2339] },
      { name: "tuple-write", source: "view.coordinates[0] = 1;", codes: [2540] },
      { name: "tuple-push", source: "view.coordinates.push(4);", codes: [2339] },
      { name: "short-tuple", source: "const coordinates: Coordinates = [1, 2];", codes: [2322] },
      { name: "long-tuple", source: "const coordinates: Coordinates = [1, 2, 3, 4];", codes: [2322] }
    ];
    const target = resolve(root, "target");
    await mkdir(target, { recursive: true });
    const directory = await mkdtemp(resolve(target, "game-ui-typecheck-"));
    try {
      await writeFile(resolve(directory, "generated.ts"), declaration.content, "utf8");
      await writeFile(resolve(directory, "nested.ts"), readonlyDeclarations(nestedDeclaration), "utf8");
      const paths = fixtures.map((fixture) => resolve(directory, `${fixture.name}.ts`));
      await Promise.all(fixtures.map(async (fixture, index) => {
        const path = paths[index];
        if (path === undefined) throw new Error("Missing compiler fixture path");
        await writeFile(path, `${imports}${bindings}${fixture.source}\n`, "utf8");
      }));
      const program = ts.createProgram(paths, { strict: true, noUncheckedIndexedAccess: true, exactOptionalPropertyTypes: true,
        target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
        types: [], noEmit: true });
      const diagnostics = ts.getPreEmitDiagnostics(program);
      expect(diagnostics.filter((diagnostic) => diagnostic.file === undefined || !paths.includes(resolve(diagnostic.file.fileName)))).toEqual([]);
      expect(fixtures.map((fixture, index) => ({ name: fixture.name,
        codes: diagnostics.filter((diagnostic) => diagnostic.file !== undefined && resolve(diagnostic.file.fileName) === paths[index])
          .map((diagnostic) => diagnostic.code) })))
        .toEqual(fixtures.map((fixture) => ({ name: fixture.name, codes: fixture.codes })));
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }, 10_000);

  it("generates deterministic standalone browser validators that reject unsafe boundaries", async () => {
    const schemas = parseSchemaBundle(schemaBundle());
    const files = await generateGameUiFiles(schemas, root);
    expect(await generateGameUiFiles(schemas, root)).toEqual(files);
    expect(files.filter((file) => file.path.startsWith("web/src/generated/")).map((file) => file.path)).toEqual([
      "web/src/generated/game-ui-input-query.ts",
      ...schemaEntries.map((entry) => `web/src/generated/${entry.filename}.ts`),
      "web/src/generated/game-ui-validators.js",
      "web/src/generated/game-ui-validators.d.ts"
    ]);
    expect(files.filter((file) => file.path.startsWith("tools/game-ui-build/generated/")).map((file) => file.path)).toEqual([
      "tools/game-ui-build/generated/game-ui-serialize.schema.json",
      "tools/game-ui-build/generated/game-ui-deserialize.schema.json",
      "tools/game-ui-build/generated/game-ui-generation.json"
    ]);
    const javascript = files.find((file) => file.path === "web/src/generated/game-ui-validators.js");
    if (javascript === undefined) throw new Error("Missing generated standalone validators");
    expect(javascript.content).not.toMatch(/\beval\s*\(|\bnew Function\s*\(|from ["']node:/);
    const module: unknown = await import(`data:text/javascript;base64,${Buffer.from(javascript.content).toString("base64")}`);
    if (typeof module !== "object" || module === null || !("validateGameUiView" in module) || typeof module.validateGameUiView !== "function") {
      throw new Error("Missing generated view validator");
    }
    const valid = { kind: "fixture", id: 4294967295, coordinates: [1, 2, 3] };
    const invalidDocuments = [
      { ...valid, id: 4294967296 },
      { ...valid, id: 0.5 },
      { ...valid, id: -1 },
      { ...valid, kind: "unknown" },
      { ...valid, extra: true },
      { ...valid, coordinates: [1, 2] },
      { ...valid, coordinates: [1, 2, 3, 4] },
      { ...valid, coordinates: [1, Number.NaN, 3] },
      { ...valid, coordinates: [1, Number.POSITIVE_INFINITY, 3] }
    ];
    for (const entry of schemaEntries) {
      const name = `validate${entry.name}`;
      const validate = (module as Readonly<Record<string, unknown>>)[name] as ValidateFunction;
      expect(typeof validate).toBe("function");
      expect(validate(valid)).toBe(true);
      for (const invalid of invalidDocuments) expect(validate(invalid)).toBe(false);
      expect(validate({ kind: "fixture", coordinates: [1, 2, 3] })).toBe(entry.group === "deserialize");
      expect(validate({ ...valid, kind: "unknown", id: -1, extra: true })).toBe(false);
      expect(validate.errors?.map((error) => error.instancePath)).toEqual(expect.arrayContaining(["/kind", "/id", ""]));
    }
    expect(javascript.content.match(/^function validate\d+\(/gm))
      .toHaveLength(schemaEntries.length + new Set(schemaEntries.map((entry) => entry.group)).size * 2);
    const license = files.find((file) => file.path === "assets/game-ui-validators.LICENSE.txt");
    expect(license?.content).toContain("Copyright (c) 2015-2021 Evgeny Poberezkin");
    expect(license?.content).toContain("Copyright (c) 2020 Evan Wallace");
    expect(license?.content).toContain("Permission is hereby granted");
  });
});
