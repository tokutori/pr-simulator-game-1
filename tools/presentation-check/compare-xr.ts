import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import type { MenuPreparationInput } from "../../web/src/generated/menu-preparation-input.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import type { MenuLayoutReply } from "../../web/src/generated/menu-layout-reply.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { DEFAULT_XR_HEAD_HUD_PROFILE } from "../../web/src/generated/xr-head-hud-profile.js";
import { prepareHeadHudPresentation } from "../../web/src/presentation/head-hud-canvas.js";
import type { HeadHudPreparationInput } from "../../web/src/generated/head-hud-preparation-input.js";
import type { HeadHudPaintReply } from "../../web/src/generated/head-hud-paint-reply.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { IDENTITY_POSE, composePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import { XrCameraBridge, XrSpatialBridge,
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
for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
  for (const depth of ["negative-one-to-one", "zero-to-one", "reversed-zero-to-one"] as const) {
    optics.push({ name: `${String(width)}x${String(height)}-${depth}`,
      observation: observation(opticalViewer(width, height, depth, true)) });
  }
}
const heads = headObservations();
const batch = { optics: optics.map(candidate => candidate.observation),
  head: heads.observations, interaction: pointerScenario(), menu: menus.observations };
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
    optics: batch.optics.map(candidate => ({ spatial: spatialResults(candidate) })),
    head: heads.results,
    interaction: pointerResults(batch.interaction),
    menu: menus.results
  };
  await writeFile(resolve(output, "wasm.json"), JSON.stringify(actual), "utf8");
  compare(expected, actual, "batch");
  assert.equal(execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim(), head, "HEAD changed during comparison");
  assert.equal(execFileSync("git", sourceArguments, gitOptions), patch, "Source changed during comparison");
  assert.equal(execFileSync("git", untrackedArguments, gitOptions), "", "New source appeared during comparison");
  assert.equal(createHash("sha256").update(await readFile(wasmPath)).digest("hex"), wasmHash, "WASM changed during comparison");
  const report = { status: "passed", ...provenance, cases: [...optics.map(candidate => candidate.name), ...heads.names], batches: { optics: optics.length, headPreparations: heads.results.length, interaction: 1, menuPreparations: batch.menu.length }, comparedNumbers, maximumAbsoluteError,
    tolerance: "error <= 1e-10 * max(1, abs(native), abs(wasm))" };
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  console.log(JSON.stringify({ status: report.status, cases: optics.length + heads.results.length, batches: report.batches, comparedNumbers, maximumAbsoluteError, output }));
} catch (cause) {
  await writeFile(resolve(output, "result.json"), `${JSON.stringify({ status: "failed", ...provenance,
    failure: cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause) }, null, 2)}\n`, "utf8");
  throw cause;
}

function headObservations() {
  const observations: { view: import("../../web/src/generated/game-ui-view.js").GameUiView;
    frame: import("../../web/src/generated/game-ui-frame.js").GameUiFrame; input: HeadHudPreparationInput; probes: TextProbe[] }[] = [];
  const results: HeadHudPaintReply[] = [];
  const names: string[] = [];
  for (const informationCode of [0, 1, 2, 3, 4] as const) {
    for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
      withSharedGameUiSession("flight_running", { informationCode, ticks: 20, platform: { presentation_mode: "hmd" } }, (session, platform, bridge) => {
        const view = session.readSharedUi(platform);
        const frame = JSON.parse(bridge.game_ui_frame_json(view.view_revision, JSON.stringify(platform))) as import("../../web/src/generated/game-ui-frame.js").GameUiFrame;
        const input: HeadHudPreparationInput = { viewRevision: view.view_revision, viewer: configuredViewerFixture(width, height),
          locale: "ja", profile: DEFAULT_XR_HEAD_HUD_PROFILE, frameRateLabel: "FPS 72.0",
          font: { family: "sans-serif", weight: 600, style: "normal", generation: 0 } };
        const probes: TextProbe[] = [];
        const reply = prepareHeadHudPresentation(new HudCanvasFixture(), input, (request, measure) =>
          session.prepareSharedHeadHud(request, platform, (value, fontMeters, weight, surfaceWidth, surfaceHeight) => {
            const metrics = measure(value, fontMeters, weight, surfaceWidth, surfaceHeight);
            probes.push({ value, font_meters: fontMeters, weight, size: { width: surfaceWidth, height: surfaceHeight }, metrics: Array.from(metrics) });
            return metrics;
          }));
        assert.equal(reply.kind, "ready");
        observations.push({ view, frame, input, probes });
        results.push(reply);
        names.push(`head-${String(informationCode)}-${String(width)}x${String(height)}`);
      });
    }
  }
  return { observations, results, names };
}

function menuObservations() {
  const observations: { view: import("../../web/src/generated/game-ui-view.js").GameUiView;
    frame: import("../../web/src/generated/game-ui-frame.js").GameUiFrame; input: MenuPreparationInput; probes: TextProbe[] }[] = [];
  const results: { preparation: MenuLayoutReply }[] = [];
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
          configuredViewerFixture(width, height), { viewRevision: view.view_revision, locale: "ja",
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
        observations.push({ view, frame, input, probes });
        results.push({ preparation: prepared });
      });
    }
  }
  return { observations, results };
}

function pointerScenario() {
  const panelPose = pose(vec3(20, 10, -30), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0));
  const sourceRay = (source: XrPointerObservation["pointers"][number]["source"], horizontal: number) => {
    const reference = composePose(panelPose, pose(vec3(horizontal, 0.1, 2), IDENTITY_POSE.orientation));
    return { source, referenceFromSpace: reference, origin: vec3(0, 0, 0), direction: vec3(0, 0, -1) };
  };
  const headPose = composePose(panelPose, pose(vec3(0, 0.1, 2), IDENTITY_POSE.orientation));
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
    timestampMs, focused, panelPose, headPose, pointers: [], gamepad: { index: 0, generation, axes: [1, -1, 0, 0.5], buttons }
  });
  const gamepadSteps = [
    gamepad(5000, 1, [true, true]), gamepad(5016, 1, [false, false]),
    gamepad(5032, 1, [true, false]), gamepad(5048, 1, [true, false]),
    gamepad(5064, 1, [false, false]), gamepad(5080, 1, [true, true], false),
    gamepad(5096, 1, [true, true]), gamepad(5112, 1, [false, false]),
    gamepad(5128, 1, [false, true]), gamepad(5144, 2, [true, true]),
    { timestampMs: 5160, focused: true, panelPose, headPose, pointers: [], gamepad: null },
    { timestampMs: 6160, focused: true, panelPose, headPose, pointers: [], gamepad: null }
  ];
  return { surface, steps: [...steps.map(([timestampMs, focused, pointers, select]) => ({
    observation: { timestampMs, focused, panelPose, headPose, pointers: pointers.filter(pointer => pointer.source.kind === "controller") } satisfies XrPointerObservation, select
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

function observation(viewer: ViewerFrame) {
  return { viewer,
    panel_pose: pose(vec3(0.1, -0.2, 0), IDENTITY_POSE.orientation), panel_size: [2.4, 1.8] as const,
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
