import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { compile } from "json-schema-to-typescript";
import ts from "typescript";

export const schemaEntries = [
  { key: "view", name: "GameUiView", filename: "game-ui-view", group: "serialize" },
  { key: "action", name: "GameUiAction", filename: "game-ui-action", group: "deserialize" },
  { key: "input", name: "GameUiPlatformInput", filename: "game-ui-input", group: "deserialize" },
  { key: "effects", name: "GameUiUpdate", filename: "game-ui-effects", group: "serialize" },
  { key: "frame", name: "GameUiFrame", filename: "game-ui-frame", group: "serialize" },
  { key: "xr_viewer_frame", name: "XrViewerFrame", filename: "xr-viewer-frame", group: "serialize" },
  { key: "xr_head_hud_profile", name: "XrHeadHudProfile", filename: "xr-head-hud-profile", group: "serialize", defaultName: "DEFAULT_XR_HEAD_HUD_PROFILE" },
  { key: "xr_flight_menu_geometry", name: "XrFlightMenuGeometry", filename: "xr-flight-menu-geometry", group: "serialize", defaultName: "XR_FLIGHT_MENU_GEOMETRY" },
  { key: "xr_head_hud_projection", name: "XrHeadHudProjection", filename: "xr-head-hud-projection", group: "serialize" },
  { key: "xr_menu_view_fit", name: "XrMenuViewFit", filename: "xr-menu-view-fit", group: "serialize" },
  { key: "xr_ink_fit", name: "XrInkFit", filename: "xr-ink-fit", group: "serialize" },
  { key: "xr_ink_segment", name: "XrInkSegment", filename: "xr-ink-segment", group: "deserialize" },
  { key: "xr_camera_rig", name: "XrCameraRig", filename: "xr-camera-rig", group: "serialize" },
  { key: "xr_pilot_eye_pose", name: "XrPilotEyePose", filename: "xr-pilot-eye-pose", group: "serialize" },
  { key: "xr_frd_pose", name: "XrFrdPose", filename: "xr-frd-pose", group: "serialize" },
  { key: "xr_panel_placement", name: "XrPanelPlacement", filename: "xr-panel-placement", group: "serialize" },
  { key: "xr_panel_camera_poses", name: "XrPanelCameraPoses", filename: "xr-panel-camera-poses", group: "serialize" },
  { key: "xr_camera_mount", name: "XrCameraMount", filename: "xr-camera-mount", group: "deserialize" },
  { key: "xr_panel_observation", name: "XrPanelObservation", filename: "xr-panel-observation", group: "deserialize" },
  { key: "xr_reference_frame", name: "XrReferenceFrame", filename: "xr-reference-frame", group: "serialize" },
  { key: "screen_render_auto", name: "ScreenRenderAutoSnapshot", filename: "screen-render-auto", group: "serialize" },
  { key: "replay_camera", name: "ReplayCameraView", filename: "replay-camera", group: "serialize" },
  { key: "tail_input", name: "TailInputDocument", filename: "tail-input", group: "deserialize" },
  { key: "tail_snapshot", name: "TailSnapshotDocument", filename: "tail-snapshot", group: "serialize" },
  { key: "tail_prepared", name: "TailPreparedLaunchDocument", filename: "tail-prepared", group: "serialize" },
  { key: "lifecycle", name: "SessionLifecycleDocument", filename: "session-lifecycle", group: "serialize" },
  { key: "tail_profile", name: "TailControlProfileDocument", filename: "tail-control-profile", group: "serialize" },
  { key: "runtime_environment", name: "EnvironmentSnapshot", filename: "runtime-environment", group: "serialize" },
  { key: "flight_render_pose", name: "FlightRenderPose", filename: "flight-render-pose", group: "serialize" },
  { key: "tail_scenario", name: "EnvironmentIdentity", filename: "tail-scenario", group: "serialize" },
  { key: "tail_identity", name: "FlightRecordTailIdentityDocument", filename: "tail-identity", group: "serialize" },
  { key: "tail_incidence", name: "TailIncidenceDocument", filename: "tail-incidence", group: "serialize" },
  { key: "tail_finalization", name: "TailFlightRecordFinalizationDocument", filename: "tail-finalization", group: "serialize" },
  { key: "record_difficulty", name: "FlightRecordDifficultyDocument", filename: "record-difficulty", group: "serialize" },
  { key: "record_context", name: "PlaybackContext", filename: "record-context", group: "serialize" },
  { key: "record_sample", name: "RecordedPlaybackSample", filename: "record-sample", group: "serialize" },
  { key: "record_analysis", name: "AnalysisDocument", filename: "record-analysis", group: "serialize" },
  { key: "record_summary", name: "SummaryDocument", filename: "record-summary", group: "serialize" },
  { key: "record_wind", name: "WindDocument", filename: "record-wind", group: "serialize" }
] as const;

const schemaGroups = ["serialize", "deserialize"] as const;
export type GameUiSchemas = Readonly<Record<typeof schemaGroups[number], Readonly<Record<string, unknown>>>>;
export type GeneratedFile = Readonly<{ path: string; content: string }>;

const generatedBanner = "Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate.";

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

export async function generateGameUiFiles(schemas: GameUiSchemas): Promise<readonly GeneratedFile[]> {
  const require = createRequire(import.meta.url);
  const files: GeneratedFile[] = [];
  const definitions = schemas.deserialize["$defs"];
  if (!isObject(definitions)) throw new Error("Missing input query definitions");
  const input = definitions["GameUiPlatformInput"];
  const records = definitions["GameUiStoredRecordPreview"];
  const record = definitions["GameUiStoredRecord"];
  const properties = isObject(input) ? input["properties"] : undefined;
  const recordProperties = isObject(record) ? record["properties"] : undefined;
  const notification = isObject(properties) ? properties["notification"] : undefined;
  const savedAt = isObject(recordProperties) ? recordProperties["saved_at"] : undefined;
  const query = {
    maximumBytes: schemaLimit(input, "x-boundary-bytes"),
    storedRecordLimit: schemaLimit(records, "maxItems"),
    savedAtCharacters: schemaLimit(savedAt, "maxLength"),
    noticeCharacters: schemaLimit(notification, "maxLength")
  };
  files.push({ path: "web/src/generated/game-ui-input-query.ts", content:
    `/* ${generatedBanner} */\nexport const gameUiInputQuery = Object.freeze(${JSON.stringify(query, null, 2)} as const);\n` });
  const sourceHashes: Record<string, string> = {};
  for (const group of schemaGroups) {
    const schema = { ...schemas[group], $id: `https://tokutori.github.io/pr-simulator-game-1/game-ui/${group}` };
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
    const schema: Record<string, unknown> = { ...definition, $defs: definitions, $schema: group["$schema"], title: entry.name };
    const declarations = await compile(schema, entry.name, {
      bannerComment: "",
      unknownAny: true,
      strictIndexSignatures: true,
      style: { singleQuote: false, tabWidth: 2, printWidth: 120, trailingComma: "none", semi: true }
    });
    if ("defaultName" in entry && !isObject(definition["default"])) throw new Error(`Missing shared default for ${entry.name}`);
    const defaults = "defaultName" in entry ? `\nexport const ${entry.defaultName} = Object.freeze(${JSON.stringify(definition["default"], null, 2)} as const);\n` : "";
    files.push({ path: `web/src/generated/${entry.filename}.ts`, content: `/* ${generatedBanner} */\n${readonlyDeclarations(declarations)}${defaults}` });
  }
  const generators = Object.fromEntries(await Promise.all(Object.entries({ jsonSchemaToTypescript: "json-schema-to-typescript", typescript: "typescript" })
    .map(async ([key, name]): Promise<readonly [string, string]> => {
      const metadata: unknown = JSON.parse(await readFile(require.resolve(`${name}/package.json`), "utf8"));
      if (!isObject(metadata) || typeof metadata["version"] !== "string") throw new Error(`Missing ${name} generator version`);
      return [key, metadata["version"]];
    })));
  files.push({ path: "tools/game-ui-build/generated/game-ui-generation.json", content: `${JSON.stringify({ schemaVersion: 2, generators, sources: sourceHashes }, null, 2)}\n` });
  return files;
}

function schemaLimit(schema: unknown, key: string): number {
  const value = isObject(schema) ? schema[key] : undefined;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`Missing positive input query limit: ${key}`);
  }
  return value;
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
