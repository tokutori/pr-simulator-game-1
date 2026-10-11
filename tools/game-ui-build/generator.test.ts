import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { generateGameUiFiles, parseSchemaBundle, readonlyDeclarations, schemaEntries } from "./generator.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

function schemaBundle(): string {
  return JSON.stringify(Object.fromEntries(schemaEntries.map((entry) => [entry.key, {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: entry.name,
    type: "object",
    additionalProperties: false,
    required: ["kind", "id", "coordinates"],
    $defs: {
      identifier: { type: "integer", format: "uint32", minimum: 0 },
      coordinates: { type: "array", minItems: 3, maxItems: 3, items: { type: "number", format: "double" } }
    },
    properties: {
      kind: { const: "fixture", type: "string" },
      id: { $ref: "#/$defs/identifier" },
      coordinates: { $ref: "#/$defs/coordinates" }
    }
  }])));
}

describe("Rust game UI schema generation", () => {
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
    expect(readonlyDeclarations("export interface Nested { values: { position: number[] }[] }"))
      .toMatch(/readonly values: readonly \{\s*readonly position: readonly number\[\]/);
  });

  it("generates deterministic standalone browser validators that reject unsafe boundaries", async () => {
    const schemas = parseSchemaBundle(schemaBundle());
    const files = await generateGameUiFiles(schemas, root);
    expect(await generateGameUiFiles(schemas, root)).toEqual(files);
    expect(files.filter((file) => file.path.startsWith("web/src/generated/")).map((file) => file.path)).toEqual([
      ...schemaEntries.map((entry) => `web/src/generated/${entry.filename}.ts`),
      "web/src/generated/game-ui-validators.js",
      "web/src/generated/game-ui-validators.d.ts"
    ]);
    expect(files.filter((file) => file.path.startsWith("tools/game-ui-build/generated/")).map((file) => file.path)).toEqual([
      ...schemaEntries.map((entry) => `tools/game-ui-build/generated/${entry.filename}.schema.json`),
      "tools/game-ui-build/generated/game-ui-generation.json"
    ]);
    const javascript = files.find((file) => file.path === "web/src/generated/game-ui-validators.js");
    if (javascript === undefined) throw new Error("Missing generated standalone validators");
    expect(javascript.content).not.toMatch(/\beval\s*\(|\bnew Function\s*\(|from ["']node:/);
    const module: unknown = await import(`data:text/javascript;base64,${Buffer.from(javascript.content).toString("base64")}`);
    if (typeof module !== "object" || module === null || !("validateGameUiView" in module) || typeof module.validateGameUiView !== "function") {
      throw new Error("Missing generated view validator");
    }
    const validate = module.validateGameUiView as (value: unknown) => boolean;
    const valid = { kind: "fixture", id: 4294967295, coordinates: [1, 2, 3] };
    expect(validate(valid)).toBe(true);
    for (const invalid of [
      { ...valid, id: 4294967296 },
      { ...valid, id: 0.5 },
      { ...valid, id: -1 },
      { ...valid, kind: "unknown" },
      { ...valid, extra: true },
      { ...valid, coordinates: [1, 2] },
      { ...valid, coordinates: [1, 2, 3, 4] },
      { ...valid, coordinates: [1, Number.NaN, 3] },
      { ...valid, coordinates: [1, Number.POSITIVE_INFINITY, 3] }
    ]) expect(validate(invalid)).toBe(false);
    const license = files.find((file) => file.path === "assets/game-ui-validators.LICENSE.txt");
    expect(license?.content).toContain("Copyright (c) 2015-2021 Evgeny Poberezkin");
    expect(license?.content).toContain("Copyright (c) 2020 Evan Wallace");
    expect(license?.content).toContain("Permission is hereby granted");
  });
});
