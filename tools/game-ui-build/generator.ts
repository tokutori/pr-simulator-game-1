import { compile } from "json-schema-to-typescript";
import ts from "typescript";

export type GeneratedFile = Readonly<{ path: string; content: string }>;

export async function generateApplicationFiles(document: unknown): Promise<readonly GeneratedFile[]> {
  if (!isObject(document) || !isObject(document["schema"]) || !isObject(document["startup"])) {
    throw new Error("Invalid shared application contracts");
  }
  const schema = document["schema"];
  const startup = document["startup"];
  if (typeof startup["loading_html"] !== "string" || typeof startup["failure_html"] !== "string") {
    throw new Error("Missing shared startup markup");
  }
  const declarations = await compile(typeReferenceSchema(schema), "ApplicationContract", {
    bannerComment: "", unknownAny: true, strictIndexSignatures: true,
    style: { singleQuote: false, tabWidth: 2, printWidth: 120, trailingComma: "none", semi: true }
  });
  return [
    { path: "web/src/generated/application-contracts.ts", content: `/* Generated from birdman-game-runtime. */\n${readonlyDeclarations(declarations)}` },
    { path: "web/src/generated/application-startup.ts", content: `/* Generated from birdman-game-runtime. */\nexport const APPLICATION_LOADING_HTML = ${JSON.stringify(startup["loading_html"])};\nexport const APPLICATION_FAILURE_HTML = ${JSON.stringify(startup["failure_html"])};\n` }
  ];
}

export function typeReferenceSchema(schema: Record<string, unknown>): Record<string, unknown> {
  if (Object.keys(schema).every((key) => ["title", "description", "default", "examples", "readOnly", "writeOnly", "deprecated"].includes(key))) {
    return { ...schema, tsType: "unknown" };
  }
  if (typeof schema["$ref"] === "string" && (schema["$ref"].startsWith("#/$defs/") || schema["$ref"].startsWith("#/definitions/"))
    && Object.keys(schema).every(key => key === "$ref" || key === "description")) return { $ref: schema["$ref"] };
  const result = { ...schema };
  for (const key of ["$defs", "definitions", "properties", "patternProperties", "dependentSchemas"]) {
    const children = schema[key];
    if (isObject(children)) result[key] = Object.fromEntries(Object.entries(children)
      .map(([name, child]) => [name, isObject(child) ? typeReferenceSchema(child) : child]));
  }
  for (const key of ["allOf", "anyOf", "oneOf", "prefixItems", "items", "additionalProperties", "additionalItems",
    "unevaluatedProperties", "unevaluatedItems", "contains", "propertyNames", "not", "if", "then", "else", "contentSchema"]) {
    const child = schema[key];
    if (isObject(child)) result[key] = typeReferenceSchema(child);
    else if (Array.isArray(child)) result[key] = child.map((value: unknown) => isObject(value) ? typeReferenceSchema(value) : value);
  }
  return result;
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
