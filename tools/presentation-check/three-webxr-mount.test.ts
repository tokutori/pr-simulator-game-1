import { IDBFactory } from "fake-indexeddb";
import { Window } from "happy-dom";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { Camera, Color, DirectionalLight, Material, Matrix4, Mesh, PerspectiveCamera, Quaternion, ShaderMaterial, Vector2, Vector3, Vector4, WebGLRenderTarget } from "three";
import type { ArrayCamera, Scene, WebGLRenderer } from "three";
import { WebXRManager } from "three/src/renderers/webxr/WebXRManager.js";
import { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationDrawingOutput, ApplicationObservation, ApplicationOutput, Pose, SurfacePaintItem } from "../../web/src/generated/application-contracts.js";
import { applicationPlatformFixture } from "../game-check/application-fixture.js";
import { gpuTimerFixture } from "./gpu-timer-fixture.js";

interface Draw {
  readonly defaultFramebuffer: boolean;
  readonly eyes: Matrix4[];
  readonly projections: Matrix4[];
  readonly panel: Matrix4 | null;
  readonly cursors: Matrix4[];
  readonly reflected: boolean;
}
interface Driver {
  readonly xr: WebXRManager;
  readonly draws: Draw[];
  readonly disposed: ReturnType<typeof vi.fn>;
  readonly timer: ReturnType<typeof gpuTimerFixture>;
  frame: ((now: number, frame?: XRFrame) => void) | null;
  compile: Promise<void>;
  scene: Scene | null;
  sizeFaults: number;
  reflectionFault: boolean;
  shaderFault: boolean;
  readonly debug: WebGLRenderer["debug"];
  getRenderTarget(): WebGLRenderTarget | null;
  getClearAlpha(): number;
  getScissorTest(): boolean;
}
const capture = vi.hoisted(() => ({ driver: null as Driver | null, compilation: null as Promise<void> | null, timing: false }));

vi.mock("three", async importOriginal => {
  const actual = await importOriginal<typeof import("three")>();
  return { ...actual, WebGLRenderer: class {
    readonly xr: WebXRManager;
    readonly draws: Draw[] = [];
    readonly disposed = vi.fn();
    readonly timer = gpuTimerFixture();
    frame: ((now: number, frame?: XRFrame) => void) | null = null;
    readonly shadowMap = { enabled: false, autoUpdate: true, needsUpdate: false };
    outputColorSpace = actual.SRGBColorSpace;
    compile = capture.compilation ?? Promise.resolve();
    scene: Scene | null = null;
    sizeFaults = 0;
    reflectionFault = false;
    shaderFault = false;
    readonly debug: WebGLRenderer["debug"] = { checkShaderErrors: true, diagnostics: { keywords: true }, onShaderError: null };
    readonly info = { programs: [] as { name: string; program: unknown; vertexShader: WebGLShader; fragmentShader: WebGLShader }[] };
    private readonly size = new actual.Vector2(1280, 720);
    private readonly viewport = new actual.Vector4(0, 0, 1280, 720);
    private readonly scissor = this.viewport.clone();
    private readonly clearColor = new actual.Color();
    private clearAlpha = 1;
    private pixelRatio = 1;
    private scissorTest = false;
    private target: WebGLRenderTarget | null = null;
    private readonly gl = { ...this.timer.gl, NO_ERROR: 0, getError: () => 0,
      LINK_STATUS: 35714, COMPILE_STATUS: 35713,
      isProgram: (program: unknown) => program instanceof WebGLProgram,
      isShader: (shader: unknown) => shader !== null && typeof shader === "object",
      isContextLost: () => false,
      getProgramParameter: () => !this.shaderFault,
      getShaderParameter: () => true,
      getProgramInfoLog: () => this.shaderFault ? "mobile program link rejected" : "",
      getShaderInfoLog: (shader: WebGLShader) => this.shaderFault && shader === this.info.programs[0]?.fragmentShader ? "mobile shader compiler cause" : null,
      getShaderSource: () => "#define SHADER_NAME LakeWater\n#define LAKE_WAVE_COUNT 8",
      getContextAttributes: () => ({ xrCompatible: true, antialias: false, depth: false, stencil: false }),
      getExtension: (name: string) => capture.timing && name === "EXT_disjoint_timer_query_webgl2" ? this.timer.gl.getExtension() : null };
    constructor() {
      this.xr = new WebXRManager(this as unknown as WebGLRenderer, this.gl as unknown as WebGLRenderingContext);
      capture.driver = this;
    }
    getContext(): unknown { return this.gl; }
    setPixelRatio(ratio: number): void { this.pixelRatio = ratio; this.setSize(this.size.x, this.size.y); }
    getPixelRatio(): number { return this.pixelRatio; }
    setSize(width: number, height: number): void {
      if (this.sizeFaults > 0) { this.sizeFaults--; throw new Error("physical framebuffer resize failed"); }
      this.size.set(width, height);
      this.viewport.set(0, 0, width * this.pixelRatio, height * this.pixelRatio);
    }
    getSize(target: Vector2): Vector2 { return target.copy(this.size); }
    getDrawingBufferSize(target: Vector2): Vector2 { return target.copy(this.size).multiplyScalar(this.pixelRatio); }
    setAnimationLoop(callback: ((now: number, frame?: XRFrame) => void) | null): void { this.frame = callback; this.xr.setAnimationLoop(callback); }
    setRenderTarget(target: WebGLRenderTarget | null): void { this.target = target; }
    getRenderTarget(): WebGLRenderTarget | null { return this.target; }
    setRenderTargetFramebuffer(): void {}
    setClearColor(color: Color | number, alpha: number): void { this.clearColor.set(color); this.clearAlpha = alpha; }
    getClearColor(target: Color): Color { return target.copy(this.clearColor); }
    getClearAlpha(): number { return this.clearAlpha; }
    setViewport(first: Vector4 | number, top?: number, width?: number, height?: number): void {
      if (first instanceof actual.Vector4) this.viewport.copy(first);
      else this.viewport.set(first, top ?? 0, width ?? 0, height ?? 0);
    }
    getViewport(target: Vector4): Vector4 { return target.copy(this.viewport); }
    setScissor(first: Vector4 | number, top?: number, width?: number, height?: number): void {
      if (first instanceof actual.Vector4) this.scissor.copy(first);
      else this.scissor.set(first, top ?? 0, width ?? 0, height ?? 0);
    }
    getScissor(target: Vector4): Vector4 { return target.copy(this.scissor); }
    setScissorTest(enabled: boolean): void { this.scissorTest = enabled; }
    getScissorTest(): boolean { return this.scissorTest; }
    initTexture(): void {}
    initRenderTarget(): void {}
    clear(): void {}
    compileAsync(scene: Scene): Promise<void> {
      this.scene = scene;
      this.info.programs.push({ name: "LakeWater", program: new WebGLProgram(), vertexShader: {}, fragmentShader: {} });
      return this.compile;
    }
    dispose(): void { this.disposed(); }
    render(scene: Scene, camera: Camera): void {
      const reflected = !this.xr.enabled && this.target !== null && !("isXRRenderTarget" in this.target && this.target.isXRRenderTarget);
      if (reflected && this.reflectionFault) throw new Error("physical reflection render failed");
      scene.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);
      const current = this.xr.enabled && this.xr.isPresenting ? this.xr.getCamera() : camera;
      let panel: Matrix4 | null = null;
      const cursors: Matrix4[] = [];
      scene.traverseVisible(object => {
        if (!(object instanceof actual.Mesh) || !current.layers.test(object.layers)) return;
        if (!(object.geometry instanceof actual.BufferGeometry)) throw new Error("Mesh geometry is unavailable");
        for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
          if (!(material instanceof actual.Material)) throw new Error("Mesh material is unavailable");
          if (this.target !== null && material instanceof actual.ShaderMaterial
            && material.uniforms["uReflectionTexture"]?.value === this.target.texture) throw new Error("Framebuffer texture feedback loop");
          material.onBeforeRender(this as unknown as WebGLRenderer, scene, camera,
            object.geometry as Parameters<Material["onBeforeRender"]>[3], object, new actual.Group());
          if (this.shaderFault && material instanceof actual.ShaderMaterial) {
            const compiled = this.info.programs[0];
            this.debug.onShaderError?.(this.gl as unknown as WebGLRenderingContext,
              compiled?.program as Parameters<NonNullable<WebGLRenderer["debug"]["onShaderError"]>>[1], compiled?.vertexShader ?? {}, compiled?.fragmentShader ?? {});
            this.shaderFault = false;
          }
        }
        if (object.geometry instanceof actual.CircleGeometry) cursors.push(object.matrixWorld.clone());
        if (object.material instanceof actual.MeshBasicMaterial && object.material.map instanceof actual.CanvasTexture && object.renderOrder === 2000) panel = object.matrixWorld.clone();
      });
      const eyes = "cameras" in current ? (current as ArrayCamera).cameras : [];
      this.draws.push({ defaultFramebuffer: this.target === null, reflected, panel, cursors, eyes: eyes.map(eye => eye.matrixWorld.clone()), projections: eyes.map(eye => eye.projectionMatrix.clone()) });
    }
  } };
});

class XrLayer {
  readonly framebuffer = {};
  readonly framebufferWidth = 1280;
  readonly framebufferHeight = 720;
  readonly ignoreDepthValues = true;
  fixedFoveation = 0;
  getViewport(view: XRView): XRViewport { return { x: view.eye === "left" ? 0 : 640, y: 0, width: 640, height: 720 }; }
}

function poseMatrix(pose: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(pose.position.x, pose.position.y, pose.position.z),
    new Quaternion(pose.orientation.x, pose.orientation.y, pose.orientation.z, pose.orientation.w), new Vector3(1, 1, 1));
}
function matrixPose(matrix: Matrix4): Pose {
  const position = new Vector3(), orientation = new Quaternion(), scale = new Vector3();
  matrix.decompose(position, orientation, scale);
  return { position: { x: position.x, y: position.y, z: position.z }, orientation: { w: orientation.w, x: orientation.x, y: orientation.y, z: orientation.z } };
}
function rigid(pose: Pose): XRRigidTransform {
  return { position: { ...pose.position, w: 1 }, orientation: { ...pose.orientation }, matrix: new Float32Array(poseMatrix(pose).elements) } as unknown as XRRigidTransform;
}
class XrSession extends EventTarget {
  readonly reference = new EventTarget();
  readonly inputSources: XRInputSource[] = [];
  readonly renderState: XRRenderStateInit = {};
  readonly enabledFeatures: readonly string[] = [];
  readonly environmentBlendMode = "opaque";
  visibilityState: XRVisibilityState = "visible";
  head: Pose = { position: { x: 0.1, y: 1.6, z: -0.1 }, orientation: { w: 1, x: 0, y: 0, z: 0 } };
  pointer: Pose | null = null;
  endCalls = 0;
  jitter = 0;
  private callback: XRFrameRequestCallback | null = null;
  requestAnimationFrame(callback: XRFrameRequestCallback): number { this.callback = callback; return 1; }
  cancelAnimationFrame(): void { this.callback = null; }
  requestReferenceSpace(): Promise<XRReferenceSpace> { return Promise.resolve(this.reference as XRReferenceSpace); }
  updateRenderState(state: XRRenderStateInit): void { Object.assign(this.renderState, state); }
  end(): Promise<void> {
    if (++this.endCalls !== 1) return Promise.reject(new Error("XR session ended twice"));
    this.dispatchEvent(new Event("end"));
    return Promise.resolve();
  }
  frame(now: number): XRFrame {
    return { predictedDisplayTime: now + 50, getViewerPose: () => ({ transform: rigid(this.head), views: [-1, 1].map((side, index) => {
      const eye = new Matrix4().compose(new Vector3(side * 0.033, 0, 0), new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), side * 0.06), new Vector3(1, 1, 1));
      const camera = new PerspectiveCamera(60, 640 / 720, 0.05, 100_000);
      camera.projectionMatrix.elements[8] = (index === 0 ? 0.16 : -0.1) + this.jitter;
      return { eye: index === 0 ? "left" : "right", projectionMatrix: new Float32Array(camera.projectionMatrix.elements), transform: rigid(matrixPose(poseMatrix(this.head).multiply(eye))) };
    }) }), getPose: () => this.pointer === null ? null : { transform: rigid(this.pointer) } } as unknown as XRFrame;
  }
  run(now: number): void {
    if (this.callback === null) throw new Error("XR animation was not started");
    this.callback(now + 50, this.frame(now));
  }
  select(now: number): void {
    const event = new Event("select");
    Object.assign(event, { frame: this.frame(now), inputSource: this.inputSources[0] });
    this.dispatchEvent(event);
  }
}

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); capture.driver = null; capture.compilation = null; capture.timing = false; });

async function fixture() {
  const browser = new Window({ url: "http://localhost/" });
  browser.document.documentElement.style.font = "400 16px Arial";
  browser.document.documentElement.lang = "ja";
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { ready: Promise.resolve() }) });
  Object.defineProperty(browser, "indexedDB", { value: new IDBFactory() });
  const session = new XrSession();
  Object.defineProperty(browser.navigator, "xr", { value: { requestSession: () => Promise.resolve(session) } });
  for (const [key, value] of Object.entries({ window: browser, document: browser.document, navigator: browser.navigator,
    HTMLElement: browser.HTMLElement, HTMLInputElement: browser.HTMLInputElement, HTMLSelectElement: browser.HTMLSelectElement,
    MutationObserver: browser.MutationObserver, getComputedStyle: browser.getComputedStyle.bind(browser), XRWebGLLayer: XrLayer,
    WebGLProgram: class { readonly [Symbol.toStringTag] = "WebGLProgram"; } })) vi.stubGlobal(key, value);
  vi.spyOn(browser.HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement) {
    const context = { canvas: this, font: "16px Arial", measureText(value: string) {
      const size = Number.parseFloat(context.font.match(/([\d.]+)px/)?.[1] ?? "16");
      return { width: value.length * size * 0.7, actualBoundingBoxLeft: 0, actualBoundingBoxRight: value.length * size * 0.7, actualBoundingBoxAscent: size * 0.8, actualBoundingBoxDescent: size * 0.2 };
    }, createLinearGradient: () => ({ addColorStop: () => undefined }) };
    return new Proxy(context, { get(target, key): unknown { return Reflect.get(target, key) ?? (() => undefined); } }) as unknown as ReturnType<typeof browser.HTMLCanvasElement.prototype.getContext>;
  });
  const updates = vi.spyOn(ApplicationRuntimeBridge.prototype, "update");
  const free = vi.spyOn(ApplicationRuntimeBridge.prototype, "free");
  const mount = browser.document.createElement("main") as unknown as HTMLElement;
  browser.document.body.append(mount as unknown as import("happy-dom").HTMLElement);
  const { BrowserApplication } = await import("../../web/src/browser-application.js");
  new BrowserApplication(mount, applicationPlatformFixture().capabilities);
  const driver = capture.driver;
  if (driver === null) throw new Error("GPU context was not created");
  const drawing = (): ApplicationDrawingOutput => {
    for (const result of [...updates.mock.results].reverse()) {
      if (result.type !== "return") continue;
      const output = JSON.parse(result.value) as ApplicationOutput;
      if (output.drawing != null) return output.drawing;
    }
    throw new Error("No aggregate drawing was published");
  };
  const click = (id: string): void => {
    const button = mount.querySelector<HTMLButtonElement>(`[data-control-id="${id}"]`);
    if (button === null) throw new Error(`Current View has no button ${id}`);
    button.click();
  };
  const dispose = (): void => {
    const event = new browser.PageTransitionEvent("pagehide");
    Object.defineProperty(event, "persisted", { value: false });
    browser.dispatchEvent(event);
  };
  return { browser, session, driver, free, updates, drawing, click, dispose,
    frame(at: number) { session.run(at); },
    select(at: number) { session.select(at); },
    async ready() {
      await flush();
      expect(mount.querySelector('[data-control-id="game-title-start"]')).not.toBeNull();
    }
  };
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  actual.elements.forEach((value, index) => { expect(value).toBeCloseTo(expected.elements[index] ?? Number.NaN, 6); });
}
async function flush(): Promise<void> {
  for (let index = 0; index < 20; index++) await Promise.resolve();
}
function text(items: readonly SurfacePaintItem[], label: string): Extract<SurfacePaintItem, { kind: "text" }> | undefined {
  for (const item of items) {
    if (item.kind === "text" && item.value === label) return item;
    if (item.kind === "plot") { const found = text(item.items, label); if (found !== undefined) return found; }
  }
  return undefined;
}

describe("Aggregate runtime through the real Three WebXRManager", () => {
  describe("tracked presentation", () => {
  let prepared: Awaited<ReturnType<typeof fixture>> | null = null;
  let materialProbe: { name: string; transparent: boolean; depthWrite: boolean } | undefined;
  let waveCount: number | undefined;
  beforeAll(async () => {
    const { ThreeApplicationRenderer } = await import("../../web/src/render/engines/three/application-renderer.js");
    const prepare = Object.getOwnPropertyDescriptor(ThreeApplicationRenderer.prototype, "prepare")?.value as InstanceType<typeof ThreeApplicationRenderer>["prepare"];
    vi.spyOn(ThreeApplicationRenderer.prototype, "prepare").mockImplementation(function (this: InstanceType<typeof ThreeApplicationRenderer>, request) {
      waveCount = request.assets.water.projection.waveCount;
      const asset = request.assets.scene?.meshes[0];
      const source = asset?.materials[0];
      if (materialProbe === undefined && asset !== undefined && source !== undefined) {
        Object.assign(source, { transparent: !source.transparent, depth_write: !source.depth_write });
        materialProbe = { name: asset.name, transparent: source.transparent, depthWrite: source.depth_write };
      }
      prepare.call(this, request);
    });
    capture.timing = true; prepared = await fixture(); await prepared.ready();
  });
  it("submits asymmetric eyes, stable reset placement, painted cursors and a real trigger action", async () => {
    const state = prepared;
    if (state === null) throw new Error("Browser setup did not complete");
    try {
      if (materialProbe === undefined) throw new Error("Material preparation was not observed");
      const mesh = state.driver.scene?.getObjectByName(materialProbe.name);
      if (!(mesh instanceof Mesh) || !(mesh.material instanceof Material)) throw new Error("Prepared material is unavailable");
      expect(mesh.material.transparent).toBe(materialProbe.transparent);
      expect(mesh.material.depthWrite).toBe(materialProbe.depthWrite);
      const water: ShaderMaterial[] = [];
      state.driver.scene?.traverse(object => {
        if (object instanceof Mesh && object.material instanceof ShaderMaterial) water.push(object.material);
      });
      expect(new Set(water).size).toBe(2);
      for (const material of water) {
        expect(material.defines["LAKE_WAVE_COUNT"]).toBe(waveCount);
        expect(material.uniforms["uWaveCount"]).toBeUndefined();
        expect(material.uniforms["uReflectionTexture"]?.value).toHaveProperty("isTexture", true);
      }
      expect(state.driver.draws.length).toBeGreaterThan(0);
      expect(state.driver.draws.some(draw => draw.defaultFramebuffer)).toBe(false);
      expect(state.driver.getRenderTarget()).toBeNull();
      expect(state.driver.getScissorTest()).toBe(false);
      if (state.driver.frame === null) throw new Error("Screen animation was not started");
      state.driver.frame(1);
      const query = state.drawing().graphics.query;
      expect(query).not.toBeNull();
      const renderer = state.drawing().graphics.resource;
      const observed = state.updates.mock.calls.length;
      state.driver.timer.gl.getQueryParameter.mockImplementationOnce(() => true).mockImplementationOnce(() => -1_000_000);
      state.driver.frame(2);
      const inputs = state.updates.mock.calls.slice(observed).map(([json]) => (JSON.parse(json) as ApplicationObservation).input);
      const completion = inputs.find(input => input.kind === "graphics" && input.observation.kind === "gpu_completed");
      expect(completion?.kind === "graphics" ? completion.observation : null).toEqual({ kind: "gpu_completed", query, gpu_work_ms: -1 });
      expect(inputs.some(input => input.kind === "renderer_failed")).toBe(false);
      expect(state.drawing().graphics.resource).toEqual(renderer);
      expect(state.driver.disposed).not.toHaveBeenCalled();
      const canvas = state.browser.document.querySelector(".presentation-canvas");
      if (canvas === null) throw new Error("Renderer canvas is unavailable");
      for (const buttons of [0, 2]) {
        const event = new state.browser.PointerEvent("pointermove", { buttons });
        Object.defineProperties(event, { movementX: { value: 3 }, movementY: { value: -4 } });
        canvas.dispatchEvent(event);
        const call = state.updates.mock.calls.at(-1);
        expect(call === undefined ? null : (JSON.parse(call[0]) as ApplicationObservation).input).toEqual({
          kind: "pointer_motion", display_resource_id: null, delta_px: [3, -4], buttons
        });
        const output = state.updates.mock.results.at(-1);
        expect(output?.type === "return" ? (JSON.parse(output.value) as ApplicationOutput).drawing : undefined).toBeNull();
      }
      state.click("boot-enter-hmd");
      await flush();
      expect(state.driver.xr.isPresenting).toBe(true);
      state.frame(10);
      await flush();
      state.frame(11);
      const opening = state.drawing();
      const menu = opening.spatial?.menu;
      if (menu?.kind !== "ready" || opening.scene.camera == null) throw new Error("Current tracked Menu is unavailable");
      const before = poseMatrix(menu.world_from_panel);
      const currentView = state.updates.mock.results
        .filter(result => result.type === "return")
        .map(result => (JSON.parse(result.value) as ApplicationOutput).drawing?.ui)
        .findLast(ui => ui?.kind === "view");
      if (currentView?.kind !== "view") throw new Error("Current shared View is unavailable");
      const start = currentView.view.panels.flatMap(panel => panel.controls).find(control => control.id === "game-title-start");
      if (start === undefined) throw new Error("Current shared View omitted its start control");
      const label = text(menu.paint.content.items, start.label);
      if (label === undefined) throw new Error("Shared painting omitted its start label");
      const point = new Vector3(label.anchor.x + label.fontMeters * 0.3 - menu.paint.size.width / 2,
        menu.paint.size.height / 2 - label.anchor.y + label.fontMeters * 0.3, 0).applyMatrix4(before);
      const origin = new Vector3().setFromMatrixPosition(poseMatrix(opening.scene.camera.world_from_head ?? state.session.head));
      const ray = new Matrix4().compose(origin, new Quaternion().setFromUnitVectors(new Vector3(0, 0, -1), point.sub(origin).normalize()), new Vector3(1, 1, 1));
      state.session.pointer = matrixPose(poseMatrix(state.session.head).multiply(poseMatrix(opening.scene.camera.world_from_head ?? state.session.head).invert()).multiply(ray));
      const squeeze = { pressed: false, touched: false };
      const xrPad = { mapping: "xr-standard", axes: [Number.NaN, 0.1, 0, 0.15],
        buttons: [{ pressed: true, touched: true }, squeeze, { pressed: false, touched: true }] };
      state.session.inputSources.push({ targetRayMode: "tracked-pointer", targetRaySpace: {}, profiles: [], handedness: "right", gripSpace: {}, gamepad: xrPad, hand: null } as unknown as XRInputSource);
      state.frame(20);
      const rawFrame = state.updates.mock.calls.map(([json]) => (JSON.parse(json) as ApplicationObservation).input)
        .findLast(input => input.kind === "frame");
      expect(rawFrame?.kind === "frame" ? rawFrame.spatial.sources : null).toEqual([{
        source_id: 1, target_ray_mode: "tracked-pointer", handedness: "right", tracking_from_ray: state.session.pointer,
        gamepad: { mapping: xrPad.mapping, axes: [null, 0.1, 0, 0.15], buttons: xrPad.buttons }
      }]);
      const hovered = state.drawing();
      const hoveredMenu = hovered.spatial?.menu;
      if (hoveredMenu?.kind !== "ready" || hoveredMenu.cursors.length !== 1) throw new Error("Real controller ray did not produce a shared cursor");
      const draw = state.driver.draws.filter(draw => !draw.reflected).at(-1);
      if (draw == null) throw new Error("GPU frame was not submitted");
      expect(draw.panel).not.toBeNull();
      expect(draw.cursors).toHaveLength(1);
      expect(draw.eyes).toHaveLength(2);
      const cursor = hoveredMenu.cursors[0];
      if (cursor === undefined) throw new Error("Missing cursor output");
      expectMatrix(draw.cursors[0] ?? new Matrix4(), poseMatrix(cursor.world_from_cursor).scale(new Vector3().setScalar(cursor.diameter_m)));
      for (const [index, eye] of (hovered.scene.camera?.eyes ?? []).entries()) {
        expectMatrix(draw.eyes[index] ?? new Matrix4(), poseMatrix(eye.world_from_eye));
        expect(draw.projections[index]?.elements).toEqual(eye.projection);
      }
      expect(state.driver.draws.some(draw => draw.reflected)).toBe(true);
      const change = new Matrix4().makeRotationY(0.1).setPosition(0.2, 0, -0.1);
      const reset = new Event("reset");
      Object.assign(reset, { transform: rigid(matrixPose(change)) });
      state.session.head = matrixPose(change.clone().invert().multiply(poseMatrix(state.session.head)));
      state.session.pointer = matrixPose(change.clone().invert().multiply(poseMatrix(state.session.pointer)));
      state.session.reference.dispatchEvent(reset);
      state.select(21);
      expect(state.browser.document.querySelector('[data-control-id="game-title-start"]')).not.toBeNull();
      state.session.jitter = 1.0e-7;
      state.frame(30);
      const resetMenu = state.drawing().spatial?.menu;
      if (resetMenu?.kind !== "ready") throw new Error("Menu did not recover after reference reset");
      expectMatrix(poseMatrix(resetMenu.world_from_panel), before);
      state.frame(31);
      state.select(32);
      expect(state.browser.document.querySelector('[data-control-id="game-setup-start"]')).not.toBeNull();
      state.frame(33);
      state.frame(34);
      squeeze.pressed = true;
      state.frame(35);
      expect(state.browser.document.querySelector('[data-control-id="game-title-start"]')).not.toBeNull();
      const target = state.driver.getRenderTarget();
      const alpha = state.driver.getClearAlpha();
      const scissor = state.driver.getScissorTest();
      const submitted = state.driver.draws.length;
      const diagnostic = vi.spyOn(console, "error").mockImplementation(() => undefined);
      state.driver.reflectionFault = true;
      state.frame(36);
      expect(state.driver.getRenderTarget()).toBe(target);
      expect(state.driver.getClearAlpha()).toBe(alpha);
      expect(state.driver.getScissorTest()).toBe(scissor);
      expect(state.driver.xr.enabled).toBe(true);
      expect(state.driver.draws).toHaveLength(submitted);
      expect(diagnostic).toHaveBeenCalledWith(expect.stringContaining("physical reflection render failed"));
      state.dispose();
      expect(state.free).not.toHaveBeenCalled();
      await flush();
      expect(state.free).toHaveBeenCalledOnce();
      expect(state.session.endCalls).toBe(1);
    } finally { state.dispose(); await flush(); await state.browser.happyDOM.abort(); }
  });
  });

  it.each(["cancelled cleanup", "commit rollback", "shader link"])("keeps shutdown alive until the actual %s terminal", async fault => {
    let complete: (() => void) | undefined;
    capture.compilation = new Promise(resolve => { complete = resolve; });
    const state = await fixture();
    const diagnostic = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const sun = state.driver.scene?.children.find(object => object instanceof DirectionalLight);
    if (!(sun instanceof DirectionalLight)) throw new Error("Prepared directional light is unavailable");
    const shadowRelease = vi.spyOn(sun.shadow, "dispose");
    const shaderError = vi.fn();
    try {
      if (fault === "cancelled cleanup") {
        vi.spyOn(WebGLRenderTarget.prototype, "dispose").mockImplementationOnce(() => { throw new Error("physical target release failed"); });
        state.dispose();
      } else if (fault === "commit rollback") state.driver.sizeFaults = 2;
      else { state.driver.shaderFault = true; state.driver.debug.onShaderError = shaderError; }
      expect(state.free).not.toHaveBeenCalled();
      if (complete === undefined) throw new Error("Deferred shader I/O was not created");
      complete();
      if (fault !== "cancelled cleanup") {
        await flush();
        if (fault === "commit rollback") expect(state.updates.mock.calls.some(([json]) => json.includes('"binding_lost"'))).toBe(true);
        else {
          expect(state.updates.mock.calls.some(([json]) => json.includes("mobile program link rejected") && json.includes("mobile shader compiler cause"))).toBe(true);
          const details = state.browser.document.querySelector("#game-boot-diagnostic");
          expect(details?.textContent).toContain("mobile program link rejected");
          expect(details?.textContent).toContain("mobile shader compiler cause");
          expect(details?.textContent).toContain('"linkStatus":false');
          expect(details?.textContent).toContain('"vertexCompileStatus":true');
          expect(details?.textContent).toContain('"vertex":null');
          expect(details?.textContent).toContain('"contextLost":false');
          expect(state.updates.mock.calls.some(([json]) => json.includes('"committed"'))).toBe(false);
          expect(state.updates.mock.calls.filter(([json]) => json.includes('"kind":"renderer_failed"'))).toHaveLength(1);
          expect(shaderError).not.toHaveBeenCalled();
          expect(state.driver.debug.onShaderError).toBe(shaderError);
          expect(state.driver.getRenderTarget()).toBeNull();
          expect(state.driver.getScissorTest()).toBe(false);
        }
        state.dispose();
      }
      await flush();
      expect(state.free).toHaveBeenCalledOnce();
      expect(state.driver.disposed).toHaveBeenCalledOnce();
      expect(shadowRelease).toHaveBeenCalled();
      if (fault === "commit rollback") expect(diagnostic).toHaveBeenCalled();
      else if (fault === "cancelled cleanup") expect(state.updates.mock.calls.some(([json]) => json.includes("physical target release failed"))).toBe(true);
    } finally { state.dispose(); await flush(); await state.browser.happyDOM.abort(); }
  });
});
