import { describe, expect, it } from "vitest";
import { initializeWasmFixture } from "../game-check/wasm-fixture.js";
import { Matrix4, PerspectiveCamera, Quaternion, Vector3 } from "three";
import { captureXrViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { projectHeadPoint } from "./viewer-fixture.js";
import { IDENTITY_POSE, vec3 } from "../../web/src/render/contracts/math.js";
import { xr_capture_viewer_frame_json, xr_text_ink_fit_json } from "../../web/pkg/birdman_game_wasm.js";

initializeWasmFixture();

describe("Same-frame platform view capture", () => {
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
    }
    const before = frame.eyes[0].projection[0];
    const mutableProjection = source.views[1]?.projectionMatrix;
    if (mutableProjection === undefined) throw new Error("Missing source projection");
    mutableProjection[0] = 99;
    expect(frame.eyes[0].projection[0]).toBe(before);
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

  it("classifies malformed, oversized and unknown-field runtime observations before fitting", () => {
    for (const packet of ["{", " ".repeat(16_385), JSON.stringify({ source: "unavailable", reason: "viewer-unavailable", trackingFromHead: null, stale: true })]) {
      expect(JSON.parse(xr_text_ink_fit_json(packet, "[]", JSON.stringify(IDENTITY_POSE), 0.35)))
        .toEqual({ kind: "unavailable", reason: "invalid-view-geometry" });
      expect(JSON.parse(xr_capture_viewer_frame_json(packet)))
        .toEqual({ source: "unavailable", reason: "invalid-view-geometry", trackingFromHead: null });
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
