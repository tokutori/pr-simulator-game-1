import { describe, expect, it } from "vitest";
import { Euler, Matrix4, PerspectiveCamera, Quaternion, StereoCamera, Vector3 } from "three";
import { currentSharedGameUiFixture, currentSharedHeadHudFixture as currentSharedHudFixture, mapSharedHudNodes, sharedHudNode, sharedHudNodes } from "../game-check/shared-game-ui-fixture.js";
import { createHeadHudView, DEFAULT_HEAD_HUD_PROFILE, headHudElementValue } from "../../web/src/presentation/head-hud-view.js";
import { validateHeadHudLayer } from "../../web/src/render/contracts/head-hud.js";
import { IDENTITY_POSE, pose, quaternion, transformPoint, vec3 } from "../../web/src/render/contracts/math.js";
import { copyProjectionMatrix, projectHeadPoint, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { convexQuadsOverlap } from "./hud-canvas-fixture.js";

const profile = Object.freeze({ telemetry: true, attitude: true, wind: true, flight_path: true, angle_of_attack: true, warnings: true });

function configuredViewer(aspect = 1280 / 720, fieldOfView = 60, near = 0.05): ViewerFrame {
  const camera = new PerspectiveCamera(fieldOfView, aspect, near, 100);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}

describe("Pure Head Flight HUD layout", () => {
  it.each(["row", "grid"] as const)("preserves %s readout grouping and instrument order across both clear-region bands", (kind) => {
    const shared = currentSharedHudFixture({ informationCode: 0 });
    const nodes = ["first", "second"].map((id) => ({ id, label: id, content: { kind: "text" as const, tone: "normal" as const, value: "共通" } }));
    const model = { ...shared, groups: shared.groups.map((group) => group.placement === "readouts"
      ? { ...group, layout: kind === "row" ? { kind: "row" as const } : { kind: "grid" as const, columns: 2 }, nodes } : group) };
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Missing grouped readout view");
    const first = view.layer.elements[0];
    const second = view.layer.elements[1];
    expect(first?.id).toBe("first");
    expect(second?.id).toBe("second");
    expect(first?.bounds.top).toBe(second?.bounds.top);
    expect(first?.bounds.left).toBeLessThan(second?.bounds.left ?? 0);
    for (const [index, element] of view.layer.elements.entries()) {
      const previous = view.layer.elements[index - 1];
      if (previous !== undefined) expect(element.bounds.top).toBeGreaterThanOrEqual(previous.bounds.top);
    }
  });

  it("projects an additional existing-kind readout and preserves node order when the labels change", () => {
    const shared = currentSharedHudFixture({ informationCode: 1 });
    const node = { id: "new-readout", label: "姿勢 PITCH", content: { kind: "text" as const, tone: "normal" as const, value: "共通表示" } };
    const model = { ...shared, groups: shared.groups.map((group, index) => ({ ...group, nodes: index === 0 ? [...group.nodes, node] : group.nodes })) };
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Missing additional readout layout");
    expect(view.layer.elements.map((element) => element.id)).toEqual(sharedHudNodes(model).map((element) => element.id));
    expect(view.layer.elements.find((element) => element.id === node.id)?.content).toBe(node.content);
    const changed = createHeadHudView(mapSharedHudNodes(model, (element) => ({ ...element, label: "変更済み" })), configuredViewer(), "ja");
    if (changed.kind !== "visible") throw new Error("Missing changed-label layout");
    expect(changed.layer.elements.map((element) => element.id)).toEqual(view.layer.elements.map((element) => element.id));
  });

  it("projects shared geometry overrides and layout opacity without deriving them from angles", () => {
    const shared = currentSharedHudFixture({ informationCode: 0 });
    const adi = { kind: "attitude" as const, horizon: { rotation_degrees: 17, pitch_offset_ratio: 0.21 }, flight_path_y_ratio: 0.73 };
    const ticks = [{ position_ratio: 0.24, major: true, label: "CUSTOM" }, { position_ratio: 0.76, major: false, label: "" }];
    const theme = { ...shared.theme, text: "#123456", panel_background: "#234567" };
    const model = mapSharedHudNodes({ ...shared, theme, layout: { ...shared.layout, opacity: 0.41 } }, (node) => {
      switch (node.content.kind) {
        case "attitude": return { ...node, content: { ...node.content, geometry: adi } };
        case "heading_tape": return { ...node, content: { ...node.content, value: "SHARED HEADING 125.75°", ticks } };
        case "linear_gauge": return { ...node, content: { ...node.content, value: "5.0°", position_ratio: 0.5 } };
        case "text":
        case "direction": return node;
      }
    });
    const before = JSON.stringify(model);
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Expected shared geometry override HUD");
    const attitude = view.layer.elements.find((element) => element.content.kind === "attitude");
    const heading = view.layer.elements.find((element) => element.content.kind === "heading_tape");
    const aoa = view.layer.elements.find((element) => element.content.kind === "linear_gauge");
    expect(attitude?.content.kind === "attitude" && attitude.content.geometry).toBe(adi);
    expect(heading?.content.kind === "heading_tape" && heading.content.ticks).toBe(ticks);
    if (heading === undefined) throw new Error("Missing shared heading element");
    expect(headHudElementValue(heading)).toBe("SHARED HEADING 125.75°");
    expect(aoa?.content).toMatchObject({ value: "5.0°", position_ratio: 0.5 });
    expect(view.layer.theme).toBe(theme);
    expect(view.layer.backgroundAlpha).toBe(0.41);
    expect(JSON.stringify(model)).toBe(before);
  });

  it("keeps shared FlightPathOnly geometry independent of attitude and horizontal angle gauges", () => {
    const shared = currentSharedHudFixture({ informationCode: 4,
      hudProfile: { ...profile, attitude: false, flight_path: true } });
    const model = mapSharedHudNodes(shared, (node) => node.content.kind === "attitude"
      ? { ...node, content: { ...node.content, value: "28°", geometry: { kind: "flight_path_only", y_ratio: 0.37 } } } : node);
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Expected shared FlightPathOnly HUD");
    expect(view.layer.elements.find((element) => element.content.kind === "attitude")?.content)
      .toMatchObject({ value: "28°", geometry: { kind: "flight_path_only", y_ratio: 0.37 } });
  });

  it("reserves a small upper-right FPS card outside the forward clear region without changing physical readouts", () => {
    const model = currentSharedHudFixture({ informationCode: 0 });
    const before = JSON.stringify(model);
    const view = createHeadHudView(model, configuredViewer(), "ja", DEFAULT_HEAD_HUD_PROFILE, 72.5);
    if (view.kind !== "visible") throw new Error("Expected Flight HUD with FPS");
    expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
    const fps = view.layer.elements.find((element) => element.id === "head-frame-rate");
    if (fps === undefined) throw new Error("Missing frame rate card");
    expect(fps).toMatchObject({ content: { kind: "text", value: "FPS 72.5" }, label: "" });
    expect(fps.bounds.left).toBeGreaterThan(0.5);
    expect(fps.bounds.width).toBeLessThan(0.25);
    for (const element of view.layer.elements.filter((element) => element !== fps)) {
      const intersects = fps.bounds.left < element.bounds.left + element.bounds.width && element.bounds.left < fps.bounds.left + fps.bounds.width &&
        fps.bounds.top < element.bounds.top + element.bounds.height && element.bounds.top < fps.bounds.top + fps.bounds.height;
      expect(intersects).toBe(false);
    }
    expect(JSON.stringify(model)).toBe(before);
  });

  it("keeps FPS available for VR Menu scenes and during measurement warmup", () => {
    for (const rate of [null, 90]) {
      const theme = currentSharedGameUiFixture().theme;
      const view = createHeadHudView(null, configuredViewer(), "ja", DEFAULT_HEAD_HUD_PROFILE, rate, theme);
      if (view.kind !== "visible") throw new Error("Expected standalone head FPS");
      expect(view.layer.anchor).toBe("head");
      expect(view.layer.theme).toBe(theme);
      expect(view.layer.backgroundAlpha).toBe(theme.hud_opacity);
      expect(view.layer.elements).toHaveLength(1);
      expect(view.layer.elements[0]).toMatchObject({ id: "head-frame-rate", content: { value: rate === null ? "FPS —" : "FPS 90.0" } });
      expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
    }
    expect(createHeadHudView(null, configuredViewer(), "ja").kind).toBe("absent");
    expect(createHeadHudView(null, configuredViewer(), "ja", DEFAULT_HEAD_HUD_PROFILE, 90).kind).toBe("absent");
  });

  it("requires and retains an explicit display locale without changing geometry", () => {
    const model = currentSharedHudFixture({ informationCode: 0 });
    const japanese = createHeadHudView(model, configuredViewer(), "ja");
    const english = createHeadHudView(model, configuredViewer(), "en-US");
    expect(japanese.kind).toBe("visible");
    expect(english.kind).toBe("visible");
    if (japanese.kind !== "visible" || english.kind !== "visible") throw new Error("Locale layout fixture missing");
    expect(japanese.layer.locale).toBe("ja");
    expect(english.layer.locale).toBe("en-US");
    expect(japanese.layer.size).toEqual(english.layer.size);
    expect(() => createHeadHudView(model, configuredViewer(), " ")).toThrow(/locale/);
  });

  it.each([
    { code: 0, kinds: ["attitude", "heading_tape", "direction", "linear_gauge"] },
    { code: 1, kinds: ["attitude", "heading_tape"] },
    { code: 2, kinds: [] },
    { code: 3, kinds: ["attitude", "heading_tape"] },
    { code: 4, kinds: ["attitude", "heading_tape", "direction", "linear_gauge"] }
  ] as const)("uses the shared Information model for level $code", ({ code, kinds }) => {
    const model = currentSharedHudFixture({ informationCode: code, ...(code === 4 ? { hudProfile: profile } : {}) });
    const before = JSON.stringify(model);
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Expected visible Flight HUD");
    expect(view.layer.elements.filter((element) => element.content.kind !== "text").map((element) => element.content.kind)).toEqual(kinds);
    expect(view.layer.elements.map((node) => node.id)).toEqual(sharedHudNodes(model).map((node) => node.id));
    for (const node of view.layer.elements) expect(node.content).toBe(sharedHudNode(model, node.id).content);
    expect(JSON.stringify(model)).toBe(before);
    if (code === 0 || code === 4) expect(view.layer.elements.find((element) => element.id === "hud-pilot-position"))
      .toMatchObject(sharedHudNode(model, "hud-pilot-position"));
    expect(Object.isFrozen(view.layer.elements)).toBe(true);
    expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
    if (code === 3) {
      const readouts = view.layer.elements.find((element) => element.id === "hud-primary-readouts");
      expect(readouts?.content.value).toContain("対象機の実機構成は未確認");
    }
  });

  it.each(Array.from({ length: 64 }, (_, mask) => ({ mask })))("keeps all Custom cues independent for mask $mask", ({ mask }) => {
    const custom = {
      telemetry: (mask & 1) !== 0, attitude: (mask & 2) !== 0, wind: (mask & 4) !== 0,
      flight_path: (mask & 8) !== 0, angle_of_attack: (mask & 16) !== 0, warnings: (mask & 32) !== 0
    };
    const model = currentSharedHudFixture({ informationCode: 4, hudProfile: custom });
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (mask === 0 || mask === 32) {
      expect(view).toEqual({ kind: "absent" });
      return;
    }
    if (view.kind !== "visible") throw new Error("Expected visible Custom HUD");
    const byId = new Map(view.layer.elements.map((element) => [element.id, element]));
    expect(byId.has("hud-attitude")).toBe(custom.attitude || custom.flight_path);
    expect(byId.has("hud-heading")).toBe(custom.attitude);
    expect(byId.has("hud-pilot-position")).toBe(custom.telemetry);
    expect(byId.has("hud-wind")).toBe(custom.wind);
    expect(byId.has("hud-angle-of-attack")).toBe(custom.angle_of_attack);
    expect(byId.has("hud-controller-failure")).toBe(false);
    if (custom.attitude) expect(byId.get("hud-attitude")?.content).toEqual(sharedHudNode(model, "hud-attitude").content);
  });

  it("preserves undefined flow angles without inventing instrument values", () => {
    const model = mapSharedHudNodes(currentSharedHudFixture({ informationCode: 0 }), (node) => node.content.kind === "linear_gauge"
      ? { ...node, content: { ...node.content, value: "迎角未定義", position_ratio: null } } : node);
    const view = createHeadHudView(model, configuredViewer(), "ja");
    if (view.kind !== "visible") throw new Error("Missing undefined-angle HUD");
    expect(view.layer.elements.find((element) => element.id === "hud-angle-of-attack")?.content).toMatchObject({ kind: "linear_gauge", value: "迎角未定義", position_ratio: null });
    expect(view.layer.elements.some((element) => element.content.kind === "attitude" || element.content.kind === "heading_tape")).toBe(true);
  });

  it("fits asymmetric canted sheared views and keeps each card inside both eye clips", () => {
    const leftProjection = new Matrix4().makePerspective(-0.09, 0.1, 0.08, -0.075, 0.1, 100);
    leftProjection.elements[4] = 0.06;
    const rightProjection = new Matrix4().makePerspective(-0.1, 0.08, 0.085, -0.08, 0.1, 100);
    const copiedLeft = copyProjectionMatrix(leftProjection.elements);
    const copiedRight = copyProjectionMatrix(rightProjection.elements);
    if (copiedLeft === null || copiedRight === null) throw new Error("Invalid fixture projection");
    const leftRotation = new Quaternion().setFromEuler(new Euler(0.08, 0.05, 0.07));
    const rightRotation = new Quaternion().setFromEuler(new Euler(-0.07, -0.04, -0.08));
    const viewer: ViewerFrame = { source: "runtime-derived", trackingFromHead: IDENTITY_POSE, eyes: [
      { eye: "left", headFromEye: pose(vec3(-0.034, 0.003, 0.002), quaternion(leftRotation.w, leftRotation.x, leftRotation.y, leftRotation.z)), projection: copiedLeft },
      { eye: "right", headFromEye: pose(vec3(0.032, -0.003, 0.004), quaternion(rightRotation.w, rightRotation.x, rightRotation.y, rightRotation.z)), projection: copiedRight }
    ] };
    const model = currentSharedHudFixture({ informationCode: 0 });
    const view = createHeadHudView(model, viewer, "ja");
    if (view.kind !== "visible") throw new Error("Expected fitted native-style HUD");
    const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const;
    const opticalWidth = Math.tan(15 * Math.PI / 180);
    const opticalHeight = Math.tan(10 * Math.PI / 180);
    for (const element of view.layer.elements) for (const eye of viewer.eyes) {
      const projection = new Matrix4().fromArray(eye.projection);
      const eyeFromHead = new Matrix4().compose(
        new Vector3(eye.headFromEye.position.x, eye.headFromEye.position.y, eye.headFromEye.position.z),
        new Quaternion(eye.headFromEye.orientation.x, eye.headFromEye.orientation.y, eye.headFromEye.orientation.z, eye.headFromEye.orientation.w),
        new Vector3(1, 1, 1)).invert();
      const opticalCone = corners.map(([horizontal, vertical]) => new Vector3(horizontal * opticalWidth, vertical * opticalHeight, -1).applyMatrix4(projection));
      const card = corners.map(([horizontal, vertical]) => new Vector3(
        (element.bounds.left + (horizontal + 1) * element.bounds.width / 2 - 0.5) * view.layer.size.width,
        (0.5 - element.bounds.top - (1 - vertical) * element.bounds.height / 2) * view.layer.size.height,
        -DEFAULT_HEAD_HUD_PROFILE.distanceMeters).applyMatrix4(eyeFromHead).applyMatrix4(projection));
      expect(convexQuadsOverlap(card, opticalCone)).toBe(false);
      for (const horizontal of [element.bounds.left, element.bounds.left + element.bounds.width]) {
        for (const vertical of [element.bounds.top, element.bounds.top + element.bounds.height]) {
          const point = transformPoint(view.layer.localPose, vec3((horizontal - 0.5) * view.layer.size.width, (0.5 - vertical) * view.layer.size.height, 0));
          const projected = projectHeadPoint(eye, point);
          expect(projected).not.toBeNull();
          expect(Math.abs(projected?.x ?? 2)).toBeLessThan(1 - DEFAULT_HEAD_HUD_PROFILE.clipMargin);
          expect(Math.abs(projected?.y ?? 2)).toBeLessThan(1 - DEFAULT_HEAD_HUD_PROFILE.clipMargin);
          expect(projected?.z).toBeGreaterThanOrEqual(-1);
          expect(projected?.z).toBeLessThanOrEqual(1);
        }
      }
    }
    expect(createHeadHudView(model, { ...viewer, trackingFromHead: pose(vec3(30, -7, 9), quaternion(0.7, 0.3, -0.4, 0.2)) }, "ja")).toEqual(view);
  });

  it.each([1280 / 720, 720 / 1280])("keeps clear angles and minimum text height when aspect is %s", (aspect) => {
    const view = createHeadHudView(currentSharedHudFixture({ informationCode: 0 }), configuredViewer(aspect), "ja");
    if (view.kind !== "visible") throw new Error("Expected phone profile fit");
    expect(Math.atan(view.layer.clearRegion.height * view.layer.size.height / (2 * DEFAULT_HEAD_HUD_PROFILE.distanceMeters)) * 180 / Math.PI).toBeCloseTo(10, 10);
    expect(Math.atan(view.textHeightMeters / DEFAULT_HEAD_HUD_PROFILE.distanceMeters) * 180 / Math.PI).toBeCloseTo(DEFAULT_HEAD_HUD_PROFILE.textHeightDegrees, 10);
    expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
  });

  it("reports geometry loss and insufficient readable area without stale-layout fallback", () => {
    const model = currentSharedHudFixture({ informationCode: 0 });
    expect(createHeadHudView(model, configuredViewer(), "ja").kind).toBe("visible");
    expect(createHeadHudView(model, unavailableViewerFrame("unsupported-view-configuration", IDENTITY_POSE), "ja")).toEqual({ kind: "unavailable", reason: "unsupported-view-configuration" });
    expect(createHeadHudView(model, configuredViewer(0.05), "ja")).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(model, configuredViewer(1, 20), "ja")).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(model, configuredViewer(1, 60, 3), "ja")).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(currentSharedHudFixture({ informationCode: 4, hudProfile: { ...profile, telemetry: false, attitude: false, wind: false, flight_path: false, angle_of_attack: false, warnings: false } }), unavailableViewerFrame("viewer-unavailable"), "ja")).toEqual({ kind: "absent" });
  });

});
