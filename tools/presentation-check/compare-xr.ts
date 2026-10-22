import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { currentSharedHeadHudFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import type { MenuPreparationInput } from "../../web/src/generated/menu-preparation-input.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import type { MenuLayoutQuery } from "../../web/src/generated/menu-layout-query.js";
import type { MenuLayoutReply } from "../../web/src/generated/menu-layout-reply.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { DEFAULT_XR_HEAD_HUD_PROFILE } from "../../web/src/generated/xr-head-hud-profile.js";
import type { FlightRecordHudProfileDocument } from "../../web/src/generated/record-difficulty.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { IDENTITY_POSE, composePose, rotateVec3, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import { menu_layout_json, xr_head_hud_projection_json, xr_text_ink_fit_json, XrCameraBridge, XrSpatialBridge,
  xr_reference_frame_json, pose_chain_json, inverse_pose_json, xr_panel_camera_poses_json, XrPointerBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { XrPointerObservation } from "../../web/src/generated/xr-pointer-observation.js";
import type { XrPointerSurface } from "../../web/src/generated/xr-pointer-surface.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { XrCameraMount } from "../../web/src/generated/xr-camera-mount.js";
import type { XrCameraRig } from "../../web/src/generated/xr-camera-rig.js";
import type { XrReferenceFrame } from "../../web/src/generated/xr-reference-frame.js";

const root = resolve(import.meta.dirname, "../..");
const output = resolve(process.argv[2] ?? resolve(root, "target/xr-comparison"));
assert.ok(process.argv.length <= 3, "Usage: npm run test:xr -- OUTPUT_DIRECTORY");
await mkdir(output, { recursive: true });
await writeFile(resolve(output, "result.json"), JSON.stringify({ status: "failed", reason: "comparison incomplete" }), "utf8");
await Promise.all(["native.json", "wasm.json", "observations.json"].map(async filename => writeFile(resolve(output, filename), "null\n", "utf8")));
const gitOptions = { cwd: root, encoding: "utf8" as const, windowsHide: true, maxBuffer: 16 * 1024 * 1024 };
const head = execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim();
const sourceArguments = ["diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD"];
const untrackedArguments = ["ls-files", "--others", "--exclude-standard"];
assert.equal(execFileSync("git", untrackedArguments, gitOptions), "", "Add new source files to the index before verification");
const patch = execFileSync("git", sourceArguments, gitOptions);
await writeFile(resolve(output, "source.patch"), patch, "utf8");
const wasmPath = resolve(root, "web/pkg/birdman_game_wasm_bg.wasm");
const wasmHash = createHash("sha256").update(await readFile(wasmPath)).digest("hex");
const optics: { name: string; observation: ReturnType<typeof observation> }[] = [];
type TextProbe = { value: string; font_meters: number; weight: number; size: { width: number; height: number }; metrics: number[] };
const menus = menuObservations();
for (const informationCode of [0, 1, 2, 3, 4] as const) {
  const model = currentSharedHeadHudFixture({ informationCode });
  for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
    for (const depth of ["negative-one-to-one", "zero-to-one", "reversed-zero-to-one"] as const) {
      const viewer = opticalViewer(width, height, depth, informationCode === 0);
      optics.push({ name: `${String(informationCode)}-${String(width)}x${String(height)}-${depth}`, observation: observation(viewer, model) });
    }
  }
}
const customHuds: { name: string; hud: ReturnType<typeof observation>["hud"] }[] = [];
for (let mask = 0; mask < 64; mask += 1) {
  const cueNames = ["telemetry", "attitude", "wind", "flight_path", "angle_of_attack", "warnings"] as const;
  const hudProfile = Object.fromEntries(cueNames.map((cue, index) => [cue, (mask & 2 ** index) !== 0])) as unknown as FlightRecordHudProfileDocument;
  const model = currentSharedHeadHudFixture({ informationCode: 4, hudProfile });
  customHuds.push({ name: `custom-${String(mask)}`, hud: { layout: model.layout, groups: model.groups } });
}
const batch = { optics: optics.map(candidate => candidate.observation),
  custom_hud: { viewer: opticalViewer(1280, 720, "negative-one-to-one", false),
    profile: DEFAULT_XR_HEAD_HUD_PROFILE, huds: customHuds.map(candidate => candidate.hud) },
  interaction: pointerScenario(), menu: menus.observations };
assert.equal(optics.length, 30);
assert.equal(customHuds.length, 64);
const input = JSON.stringify(batch);
await writeFile(resolve(output, "observations.json"), input, "utf8");
let maximumAbsoluteError = 0;
let comparedNumbers = 0;
const provenance = { sourceHead: head, sourceDiffSha256: createHash("sha256").update(patch).digest("hex"), wasmSha256: wasmHash };
try {
  const native = execFileSync("cargo", ["run", "-p", "birdman-game-presentation", "--example", "xr_layout", "--locked", "--quiet"], {
    ...gitOptions, input, stdio: ["pipe", "pipe", "inherit"]
  });
  await writeFile(resolve(output, "native.json"), native, "utf8");
  const expected: unknown = JSON.parse(native);
  const actual = {
    optics: batch.optics.map(candidate => ({
      head: JSON.parse(xr_head_hud_projection_json(JSON.stringify(candidate.hud), JSON.stringify(candidate.viewer), JSON.stringify(candidate.profile), "FPS 72.0")) as unknown,
      ink: JSON.parse(xr_text_ink_fit_json(JSON.stringify(candidate.viewer), JSON.stringify(candidate.ink), JSON.stringify(candidate.head_from_surface), 0.35)) as unknown,
      spatial: spatialResults(candidate)
    })),
    custom_hud: batch.custom_hud.huds.map(hud => JSON.parse(xr_head_hud_projection_json(JSON.stringify(hud),
      JSON.stringify(batch.custom_hud.viewer), JSON.stringify(batch.custom_hud.profile), "FPS 72.0")) as unknown),
    interaction: pointerResults(batch.interaction),
    menu: menus.results
  };
  await writeFile(resolve(output, "wasm.json"), JSON.stringify(actual), "utf8");
  compare(expected, actual, "batch");
  assert.equal(execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim(), head, "HEAD changed during comparison");
  assert.equal(execFileSync("git", sourceArguments, gitOptions), patch, "Source changed during comparison");
  assert.equal(execFileSync("git", untrackedArguments, gitOptions), "", "New source appeared during comparison");
  assert.equal(createHash("sha256").update(await readFile(wasmPath)).digest("hex"), wasmHash, "WASM changed during comparison");
  const report = { status: "passed", ...provenance, cases: [...optics.map(candidate => candidate.name), ...customHuds.map(candidate => candidate.name)], batches: { optics: optics.length, customHud: customHuds.length, interaction: 1, menuPreparations: batch.menu.length }, comparedNumbers, maximumAbsoluteError,
    tolerance: "error <= 1e-10 * max(1, abs(native), abs(wasm))" };
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  console.log(JSON.stringify({ status: report.status, cases: optics.length + customHuds.length, batches: report.batches, comparedNumbers, maximumAbsoluteError, output }));
} catch (cause) {
  await writeFile(resolve(output, "result.json"), `${JSON.stringify({ status: "failed", ...provenance,
    failure: cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause) }, null, 2)}\n`, "utf8");
  throw cause;
}

function menuObservations() {
  const observations: { view: import("../../web/src/generated/game-ui-view.js").GameUiView;
    frame: import("../../web/src/generated/game-ui-frame.js").GameUiFrame; input: MenuPreparationInput; probes: TextProbe[]; viewport: MenuLayoutQuery[] }[] = [];
  const results: { preparation: MenuLayoutReply; viewport: MenuLayoutReply[] }[] = [];
  for (const phase of ["title", "flight_setup", "briefing_ready", "flight_paused", "result", "replay"] as const) {
    for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
      withSharedGameUiSession(phase, { ticks: 20, platform: { presentation_mode: "hmd" },
        actions: phase === "result" ? [{ kind: "set_result_tab", tab: "analysis" }] : [] }, (session, platform, bridge) => {
        const view = session.readSharedUi(platform);
        const frame = JSON.parse(bridge.game_ui_frame_json(view.view_revision, JSON.stringify(platform))) as import("../../web/src/generated/game-ui-frame.js").GameUiFrame;
        const panel = createGameViewModel(view).panels[0];
        assert.ok(panel !== undefined);
        const captured: { input: MenuPreparationInput; reply: MenuLayoutReply }[] = [];
        const probes: TextProbe[] = [];
        const ready = prepareMenuPresentation(new HudCanvasFixture(), panel,
          { generation: 1, scope: { panelId: panel.id, viewKey: "comparison" } }, 0,
          configuredViewerFixture(width, height), { theme: view.theme, viewRevision: view.view_revision, locale: "ja",
            font: { family: "sans-serif", weight: 600, style: "normal", generation: 0 } }, (request, measure) => {
            const reply = session.prepareSharedMenu(request, platform, (value, fontMeters, weight, surfaceWidth, surfaceHeight) => {
              const metrics = measure(value, fontMeters, weight, surfaceWidth, surfaceHeight);
              probes.push({ value, font_meters: fontMeters, weight, size: { width: surfaceWidth, height: surfaceHeight }, metrics: Array.from(metrics) });
              return metrics;
            });
            captured.push({ input: request, reply });
            return reply;
          });
        assert.equal(ready.kind, "ready");
        const observed = captured[0];
        assert.ok(observed !== undefined);
        const { input, reply: prepared } = observed;
        assert.equal(prepared.kind, "ready");
        const geometry = prepared.geometry;
        const plan = prepared.plan;
        const rectangles = geometry.controls.flatMap(control => [control.bounds,
          ...(control.plot === null ? [] : [control.plot]), ...control.texts.flatMap(text => text.lineGeometry.map(line => line.bounds))]);
        const viewport: MenuLayoutQuery[] = [0, 0.5, 1].map(progress => ({ kind: "viewport", source: {
          surfaceSize: plan.source.surfaceSize, paddingMeters: plan.paddingMeters, gapMeters: plan.gapMeters,
          contentHeightMeters: geometry.contentHeightMeters, footerHeightMeters: geometry.footerHeightMeters, progress },
          rectangles, panel_points: [{ x: 0, y: 0 }], document_points: [{ x: 0, y: geometry.contentHeightMeters }] }));
        observations.push({ view, frame, input, probes, viewport });
        results.push({ preparation: prepared, viewport: viewport.map(query => JSON.parse(menu_layout_json(JSON.stringify(query))) as MenuLayoutReply) });
      });
    }
  }
  return { observations, results };
}

function pointerScenario() {
  const panelPose = pose(vec3(20, 10, -30), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0));
  const sourceRay = (source: XrPointerObservation["pointers"][number]["source"], horizontal: number) => {
    const reference = composePose(panelPose, pose(vec3(horizontal, 0.1, 2), IDENTITY_POSE.orientation));
    return { source, origin: reference.position, direction: rotateVec3(reference.orientation, vec3(0, 0, -1)) };
  };
  const gaze = sourceRay({ kind: "gaze" }, 0);
  const left = sourceRay({ kind: "controller", id: 7 }, -0.2);
  const right = sourceRay({ kind: "controller", id: 11 }, 0.2);
  const surface: XrPointerSurface = { scope: "comparison", sizeM: [2, 1], regions: [
    { target: { kind: "control", id: "start" }, bounds: [0, 0, 2, 1] }
  ] };
  const steps = [
    [0, true, [gaze], null], [900, true, [gaze], null],
    [950, true, [gaze, left, right], [right, 951]], [960, true, [gaze, right, left], [left, 961]],
    [2000, false, [gaze, right], [right, 2001]], [2100, true, [gaze], [right, 2101]],
    [3100, true, [gaze], null], [4200, true, [gaze], [gaze, 4201]]
  ] as const;
  const gamepad = (timestampMs: number, generation: number, buttons: readonly [boolean, boolean], focused = true): XrPointerObservation => ({
    timestampMs, focused, panelPose, pointers: [gaze], gamepad: { index: 0, generation, axes: [1, -1, 0, 0.5], buttons }
  });
  const gamepadSteps = [
    gamepad(5000, 1, [true, true]), gamepad(5016, 1, [false, false]),
    gamepad(5032, 1, [true, false]), gamepad(5048, 1, [true, false]),
    gamepad(5064, 1, [false, false]), gamepad(5080, 1, [true, true], false),
    gamepad(5096, 1, [true, true]), gamepad(5112, 1, [false, false]),
    gamepad(5128, 1, [false, true]), gamepad(5144, 2, [true, true]),
    { timestampMs: 5160, focused: true, panelPose, pointers: [gaze], gamepad: null },
    { timestampMs: 6160, focused: true, panelPose, pointers: [gaze], gamepad: null }
  ];
  return { surface, steps: [...steps.map(([timestampMs, focused, pointers, select]) => ({
    observation: { timestampMs, focused, panelPose, pointers } satisfies XrPointerObservation, select
  })), ...gamepadSteps.map(observation => ({ observation, select: null }))] };
}

function pointerResults(scenario: ReturnType<typeof pointerScenario>): unknown[] {
  const owner = new XrPointerBridge();
  try {
    owner.set_surface_json(JSON.stringify(scenario.surface));
    return scenario.steps.map((step) => ({
      frame: JSON.parse(owner.observe_json(JSON.stringify(step.observation))) as unknown,
      select: step.select === null ? null : JSON.parse(owner.select_json(JSON.stringify(step.select[0]), step.select[1])) as unknown
    }));
  } finally { owner.free(); }
}

function observation(viewer: ViewerFrame, model: ReturnType<typeof currentSharedHeadHudFixture>) {
  return { viewer, hud: { layout: model.layout, groups: model.groups }, profile: DEFAULT_XR_HEAD_HUD_PROFILE,
    panel_pose: pose(vec3(0.1, -0.2, 0), IDENTITY_POSE.orientation), panel_size: [2.4, 1.8] as const,
    head_from_surface: pose(vec3(0, -0.2, -2.4), IDENTITY_POSE.orientation),
    ink: [{ start: vec3(-0.1, 0.025, 0), end: vec3(-0.1, -0.025, 0) }, { start: vec3(0.1, 0.025, 0), end: vec3(0.1, -0.025, 0) }],
    spatial: { aircraft: pose(vec3(10, 20, -30), quaternion(Math.cos(0.2), Math.sin(0.2), 0, 0)),
      head: pose(vec3(0.1, 0.2, -0.3), quaternion(Math.cos(0.15), 0, Math.sin(0.15), 0)),
      mounts: [{ kind: "stationary", worldFromTracking: pose(vec3(1, 2, 3), IDENTITY_POSE.orientation) },
        { kind: "aircraft", aircraftFromTracking: pose(vec3(0.04, 0.15, -0.55), IDENTITY_POSE.orientation) },
        { kind: "external", worldFromTracking: pose(vec3(2, 4, 6), quaternion(Math.cos(0.1), 0, Math.sin(0.1), 0)) }] satisfies XrCameraMount[],
      resets: [pose(vec3(-1, 0.2, 0.1), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0)),
        pose(vec3(0.3, -0.1, 0.4), quaternion(Math.cos(0.1), Math.sin(0.1), 0, 0))] } };
}

function spatialResults(candidate: ReturnType<typeof observation>): unknown {
  return candidate.spatial.mounts.map(mount => {
    const camera = new XrCameraBridge();
    const spatial = new XrSpatialBridge();
    let head = candidate.spatial.head;
    const frames: unknown[] = [];
    try {
      for (let step = 0; step <= candidate.spatial.resets.length; step += 1) {
        const rig = JSON.parse(camera.camera_rig_json(JSON.stringify(candidate.spatial.aircraft), JSON.stringify(mount))) as XrCameraRig;
        const reference = JSON.parse(xr_reference_frame_json(JSON.stringify(rig), JSON.stringify(head))) as XrReferenceFrame;
        const menu = JSON.parse(spatial.place_panel_json(JSON.stringify({ kind: "ready", geometry: {
          scope: { scene: "Replay", overlay: null, panelId: "main" }, anchor: "menu", localPose: candidate.panel_pose, sizeM: candidate.panel_size
        } }), JSON.stringify(candidate.viewer), JSON.stringify(reference))) as unknown;
        const anchors = [reference.world, reference.cockpit, reference.head].map(parent =>
          JSON.parse(xr_panel_camera_poses_json(JSON.stringify(rig), JSON.stringify(composeSpatial(parent, candidate.panel_pose)))) as unknown);
        if (candidate.viewer.source === "unavailable") throw new Error("Missing binocular comparison geometry");
        const eyes = candidate.viewer.eyes.map(eye => composeSpatial(rig.worldFromTracking, head, eye.headFromEye));
        frames.push({ rig, reference, menu, anchors, eyes });
        const change = candidate.spatial.resets[step];
        if (change !== undefined) {
          camera.reset_reference_json(JSON.stringify(change));
          head = composeSpatial(JSON.parse(inverse_pose_json(JSON.stringify(change))) as Pose, head);
        }
      }
      return frames;
    } finally { spatial.free(); camera.free(); }
  });
}

function composeSpatial(...poses: readonly Pose[]): Pose {
  return JSON.parse(pose_chain_json(JSON.stringify(poses))) as Pose;
}

function opticalViewer(width: number, height: number, convention: "negative-one-to-one" | "zero-to-one" | "reversed-zero-to-one", canted: boolean): ViewerFrame {
  const viewer = configuredViewerFixture(width, height);
  if (viewer.source === "unavailable") throw new Error("Missing configured stereo geometry");
  const eyes = viewer.eyes.map((eye, index) => {
    const projection = [...eye.projection];
    if (convention !== "negative-one-to-one") for (const column of [0, 1, 2, 3]) {
      const depth = column * 4 + 2;
      const weight = projection[column * 4 + 3] ?? 0;
      projection[depth] = ((projection[depth] ?? 0) + weight) / 2;
      if (convention === "reversed-zero-to-one") projection[depth] = weight - (projection[depth] ?? 0);
    }
    const angle = canted ? (index === 0 ? 0.04 : -0.03) : 0;
    return { ...eye, projection: projection as unknown as typeof eye.projection,
      depthRange: convention === "negative-one-to-one" ? "negative-one-to-one" as const : "zero-to-one" as const,
      headFromEye: pose(eye.headFromEye.position, quaternion(Math.cos(angle / 2), 0, Math.sin(angle / 2), 0)) };
  });
  const left = eyes[0]; const right = eyes[1];
  if (left === undefined || right === undefined) throw new Error("Missing stereo eyes");
  return { ...viewer, eyes: [left, right] };
}

function compare(expected: unknown, actual: unknown, path: string): void {
  if (typeof expected === "number") {
    assert.equal(typeof actual, "number", path);
    const difference = Math.abs(expected - Number(actual));
    maximumAbsoluteError = Math.max(maximumAbsoluteError, difference);
    comparedNumbers += 1;
    assert.ok(difference <= 1e-10 * Math.max(1, Math.abs(expected), Math.abs(Number(actual))), `${path}: ${String(expected)} != ${String(actual)}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), path);
    assert.equal(actual.length, expected.length, path);
    expected.forEach((value: unknown, index: number) => { compare(value, actual[index], `${path}[${String(index)}]`); });
  } else if (expected !== null && typeof expected === "object") {
    assert.ok(actual !== null && typeof actual === "object" && !Array.isArray(actual), path);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), path);
    for (const [key, value] of Object.entries(expected)) compare(value, (actual as Record<string, unknown>)[key], `${path}.${key}`);
  } else assert.equal(actual, expected, path);
}
