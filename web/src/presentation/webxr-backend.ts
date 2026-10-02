import { composePose, IDENTITY_POSE, inversePose, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import type { BackendFrame, PresentationBackendAdapter, RendererAdapter, SelectRay, ViewportSize } from "../render/contracts/runtime.js";
import { resolveHeadHudFrame } from "../render/contracts/head-hud.js";
import type { UiActionDispatcher, UiViewModel } from "../render/contracts/ui.js";
import { MenuAnchorPlacement, placeMenuPanel, resolveAnchorPose } from "../render/anchors.js";
import { GazeDwellSelector } from "./gaze-dwell.js";
import { actionForControl, hitTestControl, intersectPanel, rangeAction } from "./panel-interaction.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "./webxr-contracts.js";

type WebXrBackendState =
  | { readonly type: "idle" }
  | { readonly type: "requesting" }
  | { readonly type: "requested" }
  | { readonly type: "starting" }
  | { readonly type: "active" }
  | { readonly type: "stopping" }
  | { readonly type: "failed"; readonly message: string };

export class WebXrPresentationBackend implements PresentationBackendAdapter {
  readonly mode = "webxr" as const;
  private state: WebXrBackendState = { type: "idle" };
  private requestGeneration = 0;
  private lastViewerPose: Pose | null = null;
  private lastPanel: UiViewModel["panels"][number] | null = null;
  private lastPanelPose: Pose | null = null;
  private referenceFromWorld: Pose = IDENTITY_POSE;
  private menuPlacement = new MenuAnchorPlacement();
  private readonly gazeDwell: GazeDwellSelector;
  private readonly selectRayHandler = (ray: SelectRay): void => { this.handleSelectRay(ray); };

  constructor(
    private readonly xrRenderer: WebXrSessionPort,
    private readonly renderer: RendererAdapter,
    private readonly viewport: () => ViewportSize,
    private readonly dispatch: UiActionDispatcher,
    private readonly onSessionEnd: () => void,
    private readonly onReferenceSpaceResetUnavailable: () => void = () => undefined
  ) {
    this.gazeDwell = new GazeDwellSelector(dispatch);
  }

  async checkAvailability(): Promise<WebXrAvailability> {
    return this.xrRenderer.checkAvailability();
  }

  requestSessionFromUserGesture(): Promise<WebXrSessionRequest> {
    if (this.state.type !== "idle" && this.state.type !== "failed") {
      return Promise.resolve({ ok: false, message: "A WebXR session is already active or stopping" });
    }
    const generation = ++this.requestGeneration;
    this.state = { type: "requesting" };
    let request: Promise<WebXrSessionRequest>;
    try {
      request = this.xrRenderer.requestSessionFromUserGesture();
    } catch (error) {
      const message = `WebXR session request failed: ${errorMessage(error)}`;
      this.state = { type: "failed", message };
      return Promise.resolve({ ok: false, message });
    }
    return request.then((result) => {
      if (generation !== this.requestGeneration) {
        if (result.ok) void this.xrRenderer.endSession().catch(() => undefined);
        return { ok: false, message: "WebXR session request was canceled" };
      }
      this.state = result.ok ? { type: "requested" } : { type: "failed", message: result.message };
      return result;
    }, (error: unknown) => {
      const message = `WebXR session request failed: ${errorMessage(error)}`;
      if (generation === this.requestGeneration) this.state = { type: "failed", message };
      return { ok: false, message };
    });
  }

  async start(): Promise<void> {
    if (this.state.type !== "requested") throw new Error("WebXR session must be requested by an explicit user action");
    this.state = { type: "starting" };
    this.menuPlacement = new MenuAnchorPlacement();
    this.referenceFromWorld = IDENTITY_POSE;
    this.xrRenderer.setSessionEndHandler(() => { this.handleUnexpectedEnd(); });
    this.xrRenderer.setReferenceSpaceResetHandler((transform) => { this.handleReferenceSpaceReset(transform); });
    this.renderer.setSelectRayHandler(this.selectRayHandler);
    try {
      await this.xrRenderer.startSession();
      if (!this.isStarting()) throw new Error("WebXR session ended during startup");
      this.state = { type: "active" };
    } catch (error) {
      this.state = { type: "failed", message: errorMessage(error) };
      this.renderer.setSelectRayHandler(null);
      try {
        await this.xrRenderer.endSession();
      } catch (cleanupError) {
        throw new Error(`WebXR start failed: ${errorMessage(error)}; session cleanup failed: ${errorMessage(cleanupError)}`, { cause: cleanupError });
      } finally {
        this.xrRenderer.setSessionEndHandler(null);
        this.xrRenderer.setReferenceSpaceResetHandler(null);
      }
      throw error;
    }
  }

  async stop(): Promise<void> {
    this.requestGeneration++;
    this.state = { type: "stopping" };
    this.renderer.setSelectRayHandler(null);
    this.xrRenderer.setSessionEndHandler(null);
    this.xrRenderer.setReferenceSpaceResetHandler(null);
    try {
      await this.xrRenderer.endSession();
    } finally {
      this.state = { type: "idle" };
      this.lastViewerPose = null;
      this.lastPanel = null;
      this.lastPanelPose = null;
      this.referenceFromWorld = IDENTITY_POSE;
      this.gazeDwell.reset();
    }
  }

  async cancelPendingRequest(): Promise<void> {
    if (this.state.type !== "requesting" && this.state.type !== "requested") return;
    this.requestGeneration++;
    this.state = { type: "stopping" };
    try {
      await this.xrRenderer.endSession();
    } finally {
      this.state = { type: "idle" };
      this.xrRenderer.setReferenceSpaceResetHandler(null);
    }
  }

  currentFrame(timestampMs: number, viewModel: UiViewModel, viewerPose: Pose | null): BackendFrame {
    const presentationViewerPose = viewerPose === null ? null : this.renderer.transformTrackingPose(viewerPose);
    this.lastViewerPose = presentationViewerPose;
    const panel = viewModel.panels[0] ?? null;
    const headPose = presentationViewerPose ?? IDENTITY_POSE;
    if (presentationViewerPose !== null) this.menuPlacement.open(presentationViewerPose, 2.4);
    const menuPose = this.menuPlacement.current() ?? placeMenuPanel(headPose, 2.4);
    const worldFromPanel = panel === null ? menuPose : resolveAnchorPose({ kind: panel.anchor, localPose: panel.localPose }, {
      world: this.referenceFromWorld,
      cockpit: this.referenceFromWorld,
      menu: menuPose,
      head: headPose
    });
    this.lastPanel = panel;
    this.lastPanelPose = worldFromPanel;
    const gazeCursor = this.updateGaze(panel, worldFromPanel, presentationViewerPose, timestampMs);
    return Object.freeze({
      timestampMs,
      headHud: resolveHeadHudFrame(viewModel.headHud, viewerPose),
      cameraPose: IDENTITY_POSE,
      panelPose: worldFromPanel,
      panel,
      panelVisible: panel !== null,
      gazeCursor,
      viewport: this.viewport()
    });
  }

  recenterMenu(): void {
    if (this.lastViewerPose !== null) this.menuPlacement.recenter(this.lastViewerPose, 2.4);
  }

  private updateGaze(
    panel: UiViewModel["panels"][number] | null,
    worldFromPanel: Pose,
    viewerPose: Pose | null,
    timestampMs: number
  ) {
    if (panel === null || viewerPose === null) return this.gazeDwell.update(null, null, timestampMs);
    const point = intersectPanel({
      origin: viewerPose.position,
      direction: rotateVec3(viewerPose.orientation, vec3(0, 0, -1))
    }, worldFromPanel);
    return this.gazeDwell.update(panel, point, timestampMs);
  }

  private handleSelectRay(ray: SelectRay): void {
    const panel = this.lastPanel;
    const panelPose = this.lastPanelPose;
    if (panel === null || panelPose === null) return;
    const point = intersectPanel(ray, panelPose);
    if (point === null) return;
    const control = hitTestControl(panel, point);
    if (control === null || this.gazeDwell.wasActivatedRecently(control.id, ray)) return;
    this.dispatch({ type: "focus", controlId: control.id });
    const action = control.kind === "range" ? rangeAction(control, panel, point) : actionForControl(control);
    if (action !== null) this.dispatch(action);
  }

  private handleUnexpectedEnd(): void {
    if (this.state.type !== "active" && this.state.type !== "starting") return;
    this.state = { type: "idle" };
    this.renderer.setSelectRayHandler(null);
    queueMicrotask(() => {
      this.gazeDwell.reset();
      this.onSessionEnd();
    });
  }

  private handleReferenceSpaceReset(previousReferenceFromNew: Pose | null): void {
    if (this.state.type !== "active" && this.state.type !== "starting") return;
    if (previousReferenceFromNew === null) {
      this.renderer.setSelectRayHandler(null);
      this.state = { type: "failed", message: "WebXR reference-space reset cannot be reconciled" };
      this.onReferenceSpaceResetUnavailable();
      return;
    }
    const transform = inversePose(previousReferenceFromNew);
    this.referenceFromWorld = composePose(transform, this.referenceFromWorld);
    this.menuPlacement.applyReferenceTransform(transform);
    this.lastViewerPose = null;
    this.lastPanelPose = null;
    this.gazeDwell.reset();
  }

  private isStarting(): boolean {
    return this.state.type === "starting";
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
