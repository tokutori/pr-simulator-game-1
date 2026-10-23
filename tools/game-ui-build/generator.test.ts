import { describe, expect, it } from "vitest";
import { generateApplicationFiles, readonlyDeclarations, typeReferenceSchema } from "./generator.js";

describe("shared application contract generation", () => {
  it("preserves readonly nesting and reference constraints", () => {
    expect(readonlyDeclarations("export interface Nested { values: { position: number[] }[] }")).toMatch(/readonly values: readonly \{\s*readonly position: readonly number\[\]/);
    expect(typeReferenceSchema({ $ref: "#/$defs/Point", description: "A point" })).toEqual({ $ref: "#/$defs/Point" });
    expect(typeReferenceSchema({ $ref: "#/$defs/Point", enum: [1] })).toEqual({ $ref: "#/$defs/Point", enum: [1] });
  });

  it("preserves aggregate references and fixed framebuffer tuples", async () => {
    const files = await generateApplicationFiles({
      schema: { type: "object", properties: { output: { $ref: "#/definitions/ApplicationOutput" } }, required: ["output"], additionalProperties: false,
        definitions: { ApplicationOutput: { type: "object", properties: { raw: { description: "Unmodified JSON value" }, extent_px: { type: "array", items: [{ type: "number" }, { type: "number" }], minItems: 2, maxItems: 2 } }, required: ["raw", "extent_px"], additionalProperties: false } } },
      startup: { loading_html: "<p>起動中</p>", failure_html: "<button>再読み込み</button>" }
    });
    const source = files[0]?.content ?? "";
    expect(source).toContain("export interface ApplicationOutput");
    expect(source).toMatch(/readonly extent_px: readonly \[\s*number,\s*number\s*\]/);
    expect(source).toContain("readonly raw: unknown");
    expect(typeReferenceSchema({ type: "object", additionalProperties: false })).toEqual({ type: "object", additionalProperties: false });
    expect(files[1]?.content).toContain("APPLICATION_FAILURE_HTML");
  });
});
