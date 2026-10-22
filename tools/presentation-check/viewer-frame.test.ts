import { describe, expect, it } from "vitest";
import { initializeWasmFixture } from "../game-check/wasm-fixture.js";
import { Matrix4, PerspectiveCamera, Quaternion, StereoCamera, Vector3 } from "three";
import { captureConfiguredViewerFrame, captureXrViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { copyProjectionMatrix, headPlaneFitsViews, projectHeadPoint, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { xr_text_ink_fit_json } from "../../web/pkg/birdman_game_wasm.js";
import type { XrInkFit } from "../../web/src/generated/xr-ink-fit.js";

initializeWasmFixture();

describe("Immutable same-frame binocular geometry", () => {
  it("copies and normalizes reordered asymmetric runtime views without changing raw center-head", () => {
    const head = new Matrix4().makeRotationY(0.4).setPosition(2, 3, -1);
    const left = new Matrix4().makeRotationY(0.06).setPosition(-0.035, 0.004, 0.002);
    const right = new Matrix4().makeRotationY(-0.04).setPosition(0.033, -0.002, 0.003);
    const leftProjection = new PerspectiveCamera(72, 0.9, 0.05, 100).projectionMatrix;
    const rightProjection = new PerspectiveCamera(66, 1.1, 0.05, 100).projectionMatrix;
    leftProjection.elements[8] = 0.12;
    leftProjection.elements[4] = 0.03;
    rightProjection.elements[8] = -0.08;
    const source = xrViewer(head, [xrView("right", head.clone().multiply(right), rightProjection), xrView("left", head.clone().multiply(left), leftProjection)]);
    const frame = captureXrViewerFrame(source);
    expect(frame.source).toBe("runtime-derived");
    if (frame.source !== "runtime-derived") throw new Error("Missing runtime geometry");
    expect(frame.eyes.map((eye) => eye.eye)).toEqual(["left", "right"]);
    expect(frame.trackingFromHead.position).toEqual({ x: 2, y: 3, z: -1 });
    const target = new Vector3(0.25, -0.13, -2.4);
    for (const [index, eye] of frame.eyes.entries()) {
      const expected = target.clone().applyMatrix4((index === 0 ? left : right).clone().invert())
        .applyMatrix4(index === 0 ? leftProjection : rightProjection);
      const actual = projectHeadPoint(eye, vec3(target.x, target.y, target.z));
      expect(actual?.x).toBeCloseTo(expected.x, 6);
      expect(actual?.y).toBeCloseTo(expected.y, 6);
      expect(actual?.z).toBeCloseTo(expected.z, 6);
      expect(Object.isFrozen(eye.projection)).toBe(true);
      expect(Object.isFrozen(eye.headFromEye.position)).toBe(true);
    }
    const before = frame.eyes[0].projection[0];
    const mutableProjection = source.views[1]?.projectionMatrix;
    if (mutableProjection === undefined) throw new Error("Missing source projection");
    mutableProjection[0] = 99;
    expect(frame.eyes[0].projection[0]).toBe(before);
    expect(Object.isFrozen(frame)).toBe(true);
    expect(Object.isFrozen(frame.eyes)).toBe(true);
  });

  it.each([[], ["none"], ["left", "left"], ["left", "right", "none"]].map((eyes) => ({ eyes })))("reports unsupported eye arrangement $eyes with valid tracking preserved", ({ eyes }) => {
    const projection = new PerspectiveCamera().projectionMatrix;
    const source = xrViewer(new Matrix4().makeTranslation(0, 1.6, 0), eyes.map((eye) => xrView(eye as XREye, new Matrix4(), projection)));
    expect(captureXrViewerFrame(source)).toMatchObject({ source: "unavailable", reason: "unsupported-view-configuration", trackingFromHead: { position: { y: 1.6 } } });
  });

  it("separates missing tracking, invalid eye geometry, and degenerate projection", () => {
    expect(captureXrViewerFrame(null)).toEqual(unavailableViewerFrame("viewer-unavailable"));
    const projection = new PerspectiveCamera().projectionMatrix;
    const source = xrViewer(new Matrix4(), [xrView("left", new Matrix4(), projection), xrView("right", new Matrix4(), new Matrix4().set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))]);
    expect(captureXrViewerFrame(source)).toEqual(unavailableViewerFrame("invalid-view-geometry", IDENTITY_POSE));
    const invalid = xrViewer(new Matrix4(), [xrView("left", new Matrix4(), projection), xrView("right", new Matrix4(), projection)]);
    Object.assign(invalid.transform.orientation, { w: Number.NaN });
    expect(captureXrViewerFrame(invalid)).toEqual(unavailableViewerFrame("invalid-view-geometry"));
    expect(copyProjectionMatrix([1, 2])).toBeNull();
    expect(copyProjectionMatrix(Array.from({ length: 16 }, () => Number.NaN))).toBeNull();
  });

  it("keeps each frame independent across reference transforms and changing projection", () => {
    const projection = new PerspectiveCamera(60, 1, 0.1, 100).projectionMatrix;
    const eyeTransforms = [-0.032, 0.032].map((offset) => new Matrix4().makeTranslation(offset, 0, 0));
    const capture = (head: Matrix4) => captureXrViewerFrame(xrViewer(head, eyeTransforms.map((eye, index) => xrView(index === 0 ? "left" : "right", head.clone().multiply(eye), projection))));
    const initial = capture(new Matrix4());
    const transformed = capture(new Matrix4().makeRotationX(0.5).setPosition(10, -3, 2));
    if (initial.source !== "runtime-derived" || transformed.source !== "runtime-derived") throw new Error("Missing runtime frame");
    for (const index of [0, 1] as const) {
      const actual = projectHeadPoint(transformed.eyes[index], vec3(0.2, 0.3, -2));
      const expected = projectHeadPoint(initial.eyes[index], vec3(0.2, 0.3, -2));
      expect(actual?.x).toBeCloseTo(expected?.x ?? Number.NaN, 10);
      expect(actual?.y).toBeCloseTo(expected?.y ?? Number.NaN, 10);
      expect(actual?.z).toBeCloseTo(expected?.z ?? Number.NaN, 10);
    }
    projection.elements[0] = 4;
    const changed = capture(new Matrix4());
    expect(changed).not.toEqual(initial);
    expect(initial.eyes[0].projection[0]).not.toBe(4);
  });

  it("evaluates all four corners in both eyes and rejects behind-eye, clipped, and unavailable planes", () => {
    const camera = new PerspectiveCamera(60, 1280 / 720, 0.1, 100);
    camera.updateMatrixWorld(true);
    const stereo = new StereoCamera();
    stereo.aspect = 0.5;
    const frame = captureConfiguredViewerFrame(camera, stereo);
    const local = pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation);
    expect(frame.source).toBe("configured");
    expect(frame.trackingFromHead).toBeNull();
    expect(headPlaneFitsViews(frame, local, 2, 2.1, 0.03)).toBe(true);
    expect(headPlaneFitsViews(frame, local, 3, 2.1)).toBe(false);
    expect(headPlaneFitsViews(frame, pose(vec3(0, 0, 1), IDENTITY_POSE.orientation), 1, 1)).toBe(false);
    expect(headPlaneFitsViews(frame, pose(vec3(0, 0, -0.05), IDENTITY_POSE.orientation), 0.01, 0.01)).toBe(false);
    expect(headPlaneFitsViews(frame, local, 2, 2, Number.NaN)).toBe(false);
    expect(headPlaneFitsViews(unavailableViewerFrame("viewer-unavailable"), local, 1, 1)).toBe(false);
  });

  it.each(["negative-one-to-one", "zero-to-one", "reversed-zero-to-one"] as const)("uses the runtime %s clip depth in actual WASM", (convention) => {
    const matrix = new PerspectiveCamera(60, 1, 0.1, 100).projectionMatrix;
    if (convention !== "negative-one-to-one") {
      for (const column of [0, 1, 2, 3]) {
        const depth = column * 4 + 2;
        const weight = matrix.elements[column * 4 + 3] ?? 0;
        matrix.elements[depth] = ((matrix.elements[depth] ?? 0) + weight) / 2;
        if (convention === "reversed-zero-to-one") matrix.elements[depth] = weight - (matrix.elements[depth] ?? 0);
      }
    }
    const projection = copyProjectionMatrix(matrix.elements);
    if (projection === null) throw new Error("Invalid fixture projection");
    const eye = { eye: "left" as const, headFromEye: IDENTITY_POSE, projection,
      depthRange: convention === "negative-one-to-one" ? "negative-one-to-one" as const : "zero-to-one" as const };
    const frame: ViewerFrame = { source: "configured", trackingFromHead: null, eyes: [eye, { ...eye, eye: "right" }] };
    const point = vec3(0.1, -0.2, -2.4);
    const expected = new Vector3(point.x, point.y, point.z).applyMatrix4(matrix);
    const actual = projectHeadPoint(eye, point);
    expect(actual?.x).toBeCloseTo(expected.x, 12);
    expect(actual?.y).toBeCloseTo(expected.y, 12);
    expect(actual?.z).toBeCloseTo(expected.z, 12);
    for (const [distance, visible] of [[0.05, false], [2.4, true], [101, false], [-1, false]] as const) {
      expect(headPlaneFitsViews(frame, pose(vec3(0, 0, -distance), IDENTITY_POSE.orientation), 0.001, 0.001)).toBe(visible);
    }
    const surface = pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation);
    const result = JSON.parse(xr_text_ink_fit_json(JSON.stringify(frame), JSON.stringify([
      { start: vec3(0, 0.02, 0), end: vec3(0, -0.02, 0) }
    ]), JSON.stringify(surface), 0.35)) as XrInkFit;
    expect(result.kind).toBe("ready");
    if (result.kind === "ready") expect(result.minimumInkAngleDegrees).toBeCloseTo(2 * Math.atan(0.02 / 2.4) * 180 / Math.PI, 12);
  });

  it("classifies malformed, oversized and unknown-field runtime observations before fitting", () => {
    for (const packet of ["{", " ".repeat(16_385), JSON.stringify({ source: "unavailable", reason: "viewer-unavailable", trackingFromHead: null, stale: true })]) {
      expect(JSON.parse(xr_text_ink_fit_json(packet, "[]", JSON.stringify(IDENTITY_POSE), 0.35)))
        .toEqual({ kind: "unavailable", reason: "invalid-view-geometry" });
    }
  });
});

function xrViewer(head: Matrix4, views: XRView[]): XRViewerPose {
  return { transform: xrTransform(head), views, emulatedPosition: false };
}

function xrView(eye: XREye, transform: Matrix4, projection: Matrix4): XRView {
  return { eye, transform: xrTransform(transform), projectionMatrix: new Float32Array(projection.elements) } as XRView;
}

function xrTransform(matrix: Matrix4): XRRigidTransform {
  const position = new Vector3();
  const rotation = new Quaternion();
  matrix.decompose(position, rotation, new Vector3());
  return { position: { x: position.x, y: position.y, z: position.z, w: 1 }, orientation: { x: rotation.x, y: rotation.y, z: rotation.z, w: rotation.w } } as unknown as XRRigidTransform;
}
