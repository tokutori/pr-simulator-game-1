import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import ts from "typescript";
import { generateGameUiFiles, parseSchemaBundle, readonlyDeclarations, schemaEntries, typeReferenceSchema } from "./generator.js";

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
        ...("defaultName" in entry ? { default: { kind: "fixture", coordinates: [1, 2, 3], id: 0 } } : {}),
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
  it("reuses canonical root types after checking their transitive wire definitions", async () => {
    const document = JSON.parse(schemaBundle()) as Record<"serialize" | "deserialize", { $defs: Record<string, Record<string, unknown>> }>;
    const output = document.serialize.$defs;
    const action = document.deserialize.$defs["GameUiAction"];
    const view = output["GameUiView"];
    if (action === undefined || view === undefined) throw new Error("Missing canonical root fixture");
    output["GameUiAction"] = action;
    const properties = view["properties"] as Record<string, unknown>;
    properties["back_action"] = { $ref: "#/$defs/GameUiAction", description: "Back operation at this view." };
    const schemas = parseSchemaBundle(JSON.stringify(document));
    const original = JSON.stringify(schemas);
    const files = await generateGameUiFiles(schemas);
    expect(files.find(file => file.path === "web/src/generated/game-ui-view.ts")?.content)
      .toContain('import("./game-ui-action.js").GameUiAction');
    expect(JSON.stringify(schemas)).toBe(original);
    output["identifier"] = { type: "string" };
    const divergent = await generateGameUiFiles(parseSchemaBundle(JSON.stringify(document)));
    expect(divergent.find(file => file.path === "web/src/generated/game-ui-view.ts")?.content)
      .not.toContain('import("./game-ui-action.js").GameUiAction');
  });

  it("shares described references while preserving constraints and literal data", async () => {
    const schemas = JSON.parse(schemaBundle()) as Record<"serialize" | "deserialize", { $defs: Record<string, Record<string, unknown>> }>;
    schemas.serialize.$defs["Point"] = { type: "object", description: "Canonical point.", additionalProperties: false,
      required: ["position"], properties: { position: { type: "number" } } };
    schemas.serialize.$defs["GameUiView"] = { type: "object", additionalProperties: false,
      required: ["left", "right", "bounded", "default", "literal"], properties: {
        left: { $ref: "#/$defs/Point", description: "Left point." },
        right: { $ref: "#/$defs/Point", description: "Right point." },
        bounded: { $ref: "#/$defs/identifier", description: "Constrained identifier.", enum: [1] },
        default: { $ref: "#/$defs/Point", description: "A property named default." },
        literal: { const: { $ref: "#/$defs/Point", description: "Literal payload." }, type: "object" }
      } };
    const original = JSON.stringify(schemas);
    const normalized = typeReferenceSchema(schemas.serialize.$defs["GameUiView"]);
    const properties = normalized["properties"] as Record<string, unknown>;
    expect(properties["bounded"]).toEqual({ $ref: "#/$defs/identifier", description: "Constrained identifier.", enum: [1] });
    expect(properties["literal"]).toEqual({ const: { $ref: "#/$defs/Point", description: "Literal payload." }, type: "object" });
    const files = await generateGameUiFiles(parseSchemaBundle(original));
    const declaration = files.find(file => file.path === "web/src/generated/game-ui-view.ts")?.content;
    expect(declaration).toContain("readonly left: Point;");
    expect(declaration).toContain("readonly right: Point;");
    expect(declaration).toContain("readonly default: Point;");
    expect(declaration).toContain("Canonical point.");
    expect(declaration).toContain("readonly bounded: 1;");
    expect(declaration?.match(/export interface Point\b/g)).toHaveLength(1);
    expect(JSON.stringify(schemas)).toBe(original);
    expect(files.find(file => file.path.endsWith("game-ui-serialize.schema.json"))?.content).toContain("Right point.");
  });

  it("rejects mismatched schema roots", () => {
    expect(() => parseSchemaBundle("{}")).toThrow("unexpected schema roots");
    expect(() => parseSchemaBundle(schemaBundle().replace("GameUiView", "OtherView"))).toThrow("Invalid GameUiView");
  });

  it("generates deeply readonly declarations and fixed-size tuples", async () => {
    const files = await generateGameUiFiles(parseSchemaBundle(schemaBundle()));
    const declaration = files.find((file) => file.path === "web/src/generated/game-ui-view.ts");
    expect(declaration?.content).toContain('readonly kind: "fixture"');
    expect(declaration?.content).toContain("readonly coordinates: Coordinates");
    expect(declaration?.content).toContain("type Coordinates = readonly [");
    expect(declaration?.content).not.toContain("[k: string]");
    expect(readonlyDeclarations(nestedDeclaration))
      .toMatch(/readonly values: readonly \{\s*readonly position: readonly number\[\]/);
  });

  it("exports profile defaults from Rust schemas and requires their presence", async () => {
    const schemas = parseSchemaBundle(schemaBundle());
    const files = await generateGameUiFiles(schemas);
    for (const entry of schemaEntries) {
      if (!("defaultName" in entry)) continue;
      const generated = files.find(file => file.path === `web/src/generated/${entry.filename}.ts`);
      expect(generated?.content).toContain(`export const ${entry.defaultName} = Object.freeze(`);
      expect(generated?.content).toContain('"coordinates": [\n    1,\n    2,\n    3\n  ]');
    }
    const missingDefault = schemaBundle().replaceAll('"default":{"kind":"fixture","coordinates":[1,2,3],"id":0},', "");
    await expect(generateGameUiFiles(parseSchemaBundle(missingDefault))).rejects.toThrow("Missing shared default");
  });

  it("checks generated readonly and tuple contracts with the TypeScript compiler", async () => {
    const files = await generateGameUiFiles(parseSchemaBundle(schemaBundle()));
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

  it("generates deterministic types and schema without runtime validators", async () => {
    const schemas = parseSchemaBundle(schemaBundle());
    const files = await generateGameUiFiles(schemas);
    expect(await generateGameUiFiles(schemas)).toEqual(files);
    expect(files.filter((file) => file.path.startsWith("web/src/generated/")).map((file) => file.path)).toEqual([
      "web/src/generated/game-ui-input-query.ts",
      ...schemaEntries.map((entry) => `web/src/generated/${entry.filename}.ts`)
    ]);
    expect(files.filter((file) => file.path.startsWith("tools/game-ui-build/generated/")).map((file) => file.path)).toEqual([
      "tools/game-ui-build/generated/game-ui-serialize.schema.json",
      "tools/game-ui-build/generated/game-ui-deserialize.schema.json",
      "tools/game-ui-build/generated/game-ui-generation.json"
    ]);
    expect(files.some((file) => file.path.endsWith("validators.js") || file.path.endsWith("validators.d.ts")
      || file.path.endsWith("validators.LICENSE.txt"))).toBe(false);
  });
});
