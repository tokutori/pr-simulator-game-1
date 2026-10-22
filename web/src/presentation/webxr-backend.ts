import type { PresentationObservation } from "../generated/presentation-observation.js";
import { IDENTITY_POSE } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import type { BackendFrame, MenuPresentation, PresentationBackendAdapter, RendererAdapter, SelectRay, ViewportSize } from "../render/contracts/runtime.js";
import { NO_HEAD_HUD, resolveHeadHudFrame } from "../render/contracts/head-hud.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiActionDispatcher, UiViewModel } from "../render/contracts/ui.js";
import { XrSpatialSession } from "./xr-spatial-session.js";
import { XrPointerSession } from "./xr-pointer-session.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "./webxr-contracts.js";

type XrAvailability = NonNullable<Extract<PresentationObservation, { kind: "xr" }>["observation"]>;

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
  private lastAvailability: XrAvailability | null = null;
  private readonly spatial = new XrSpatialSession();
  private readonly pointers: XrPointerSession;
  private readonly selectRayHandler = (ray: SelectRay): void => { this.pointers.select(ray); };

  constructor(
    private readonly xrRenderer: WebXrSessionPort,
    private readonly renderer: RendererAdapter,
    private readonly viewport: () => ViewportSize,
    dispatch: UiActionDispatcher,
    private readonly onSessionEnd: () => void,
    private readonly onReferenceSpaceResetUnavailable: () => void = () => undefined,
    private readonly onAvailability: (observation: XrAvailability | null) => void = () => undefined
  ) {
    this.pointers = new XrPointerSession(dispatch);
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
    this.spatial.reset();
    this.lastViewerPose = null;
    const generation = this.requestGeneration;
    this.xrRenderer.setSessionEndHandler(() => {
      if (generation === this.requestGeneration) this.handleUnexpectedEnd();
    });
    this.xrRenderer.setReferenceSpaceResetHandler((transform) => {
      if (generation === this.requestGeneration) this.handleReferenceSpaceReset(transform);
    });
    this.xrRenderer.setVisibilityHandler((visibility) => {
      if (generation !== this.requestGeneration) return;
      if (this.state.type !== "active" && this.state.type !== "starting") return;
      this.publishAvailability(visibility, visibility === "visible", this.lastViewerPose !== null);
      if (visibility === "visible") return;
      this.pointers.reset();
    });
    this.renderer.setSelectRayHandler(this.selectRayHandler);
    try {
      await this.xrRenderer.startSession();
      if (!this.isStarting()) throw new Error("WebXR session ended during startup");
      this.state = { type: "active" };
      const observation = this.xrRenderer.readPointerFrame();
      this.publishAvailability(observation.visibility, observation.focused, false);
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
        this.xrRenderer.setVisibilityHandler(null);
        this.releaseAvailability();
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
    this.xrRenderer.setVisibilityHandler(null);
    try {
      await this.xrRenderer.endSession();
    } finally {
      this.state = { type: "idle" };
      this.lastViewerPose = null;
      this.spatial.reset();
      this.pointers.reset();
      this.releaseAvailability();
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

  observeViewerFrame(viewer: ViewerFrame): void {
    const input = this.xrRenderer.readPointerFrame();
    this.publishAvailability(input.visibility, input.focused, viewer.trackingFromHead !== null);
  }

  currentFrame(timestampMs: number, viewModel: UiViewModel, viewer: ViewerFrame, menu: MenuPresentation): BackendFrame {
    const viewerPose = viewer.trackingFromHead;
    const input = this.xrRenderer.readPointerFrame();
    const reference = viewerPose === null ? null : this.xrRenderer.projectTrackingFrame(viewerPose);
    const presentationViewerPose = reference?.head ?? null;
    this.lastViewerPose = presentationViewerPose;
    const semanticPanel = viewModel.panels[0] ?? null;
    const panel = menu.kind === "ready" ? menu.panel : semanticPanel;
    if (panel?.anchor !== "menu" || menu.kind === "absent") this.spatial.closeMenu();
    if (panel?.anchor === "menu" && menu.kind !== "ready") {
      this.pointers.reset();
      return Object.freeze({ timestampMs, headHud: menu.kind === "unavailable" ? NO_HEAD_HUD : resolveHeadHudFrame(viewModel.headHud, viewerPose),
        cameraPose: IDENTITY_POSE, viewport: this.viewport(), panel: menu.kind === "unavailable"
          ? Object.freeze({ kind: "unavailable", reason: menu.reason }) : Object.freeze({ kind: "absent" }) });
    }
    if (panel !== null && (presentationViewerPose === null || viewer.source === "unavailable")) {
      this.pointers.reset();
      return Object.freeze({ timestampMs, headHud: NO_HEAD_HUD, cameraPose: IDENTITY_POSE,
        panel: Object.freeze({ kind: "unavailable", reason: viewer.source === "unavailable" ? viewer.reason : "viewer-unavailable" }), viewport: this.viewport() });
    }
    const placement = this.spatial.place(viewModel, panel, viewer, reference);
    if (placement.kind === "unavailable") {
        this.pointers.reset();
        return Object.freeze({ timestampMs, headHud: NO_HEAD_HUD, cameraPose: IDENTITY_POSE, panel: placement, viewport: this.viewport() });
    }
    const worldFromPanel = placement.kind === "ready" ? placement.referenceFromPanel : IDENTITY_POSE;
    const cursors = this.pointers.observe(
      worldFromPanel, menu, presentationViewerPose, input.rays, input.focused, timestampMs);
    return Object.freeze({
      timestampMs,
      headHud: resolveHeadHudFrame(viewModel.headHud, viewerPose),
      cameraPose: IDENTITY_POSE,
      panel: panel === null ? Object.freeze({ kind: "absent" }) : Object.freeze({ kind: "visible", panel, pose: worldFromPanel, ...cursors }),
      viewport: this.viewport()
    });
  }

  recenterMenu(): void {
    if (this.lastViewerPose !== null) {
      this.spatial.recenterMenu(this.lastViewerPose);
      this.pointers.reset();
    }
  }

  private publishAvailability(visibility: XRVisibilityState, focused: boolean, tracked: boolean): void {
    if (this.state.type !== "active" && this.state.type !== "starting") return;
    const observation: XrAvailability = { visibility: visibility === "visible-blurred" ? "blurred" : visibility,
      input_focused: focused, tracking_valid: tracked };
    const previous = this.lastAvailability;
    if (previous !== null && previous.visibility === observation.visibility && previous.input_focused === focused
        && previous.tracking_valid === tracked) return;
    this.lastAvailability = observation;
    this.onAvailability(observation);
  }

  private releaseAvailability(): void {
    if (this.lastAvailability === null) return;
    this.lastAvailability = null;
    this.onAvailability(null);
  }

  private handleUnexpectedEnd(): void {
    if (this.state.type !== "active" && this.state.type !== "starting") return;
    this.publishAvailability("hidden", false, false);
    this.state = { type: "idle" };
    this.renderer.setSelectRayHandler(null);
    this.xrRenderer.setVisibilityHandler(null);
    queueMicrotask(() => {
      this.spatial.reset();
      this.pointers.reset();
      this.onSessionEnd();
    });
  }

  private handleReferenceSpaceReset(previousReferenceFromNew: Pose | null): void {
    if (this.state.type !== "active" && this.state.type !== "starting") return;
    if (previousReferenceFromNew === null) {
      const input = this.xrRenderer.readPointerFrame();
      this.publishAvailability(input.visibility, input.focused, false);
      this.renderer.setSelectRayHandler(null);
      this.state = { type: "failed", message: "WebXR reference-space reset cannot be reconciled" };
      this.onReferenceSpaceResetUnavailable();
      return;
    }
    this.lastViewerPose = null;
    this.pointers.reset();
  }

  private isStarting(): boolean {
    return this.state.type === "starting";
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
