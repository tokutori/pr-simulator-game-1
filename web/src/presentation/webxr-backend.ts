import { composePose, IDENTITY_POSE, inversePose, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import type { BackendFrame, MenuPresentation, PresentationBackendAdapter, RendererAdapter, SelectRay, ViewportSize } from "../render/contracts/runtime.js";
import { NO_HEAD_HUD, resolveHeadHudFrame } from "../render/contracts/head-hud.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiActionDispatcher, UiViewModel } from "../render/contracts/ui.js";
import { placeMenuPanel, resolveAnchorPose } from "../render/anchors.js";
import { CLOSED_MENU_PLACEMENT, openMenuPlacement, recenterMenuPlacement, transformMenuPlacement } from "./menu-placement.js";
import type { MenuPlacementModel } from "./menu-placement.js";
import { GazeDwellSelector } from "./gaze-dwell.js";
import { actionForControl, hitTestControl, intersectPanel, rangeAction } from "./panel-interaction.js";
import { menuActionAt, menuFocusAction, menuHitIdentity } from "./menu-interaction.js";
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
  private lastMenu: MenuPresentation = Object.freeze({ kind: "absent" });
  private referenceFromWorld: Pose = IDENTITY_POSE;
  private menuPlacement: MenuPlacementModel = CLOSED_MENU_PLACEMENT;
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
    this.menuPlacement = CLOSED_MENU_PLACEMENT;
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

  currentFrame(timestampMs: number, viewModel: UiViewModel, viewer: ViewerFrame, menu: MenuPresentation): BackendFrame {
    const viewerPose = viewer.trackingFromHead;
    const presentationViewerPose = viewerPose === null ? null : this.xrRenderer.transformTrackingPose(viewerPose);
    this.lastViewerPose = presentationViewerPose;
    const semanticPanel = viewModel.panels[0] ?? null;
    const panel = menu.kind === "ready" ? menu.panel : semanticPanel;
    if (panel?.anchor !== "menu" || menu.kind === "absent") this.menuPlacement = CLOSED_MENU_PLACEMENT;
    this.lastMenu = menu;
    if (panel?.anchor === "menu" && menu.kind !== "ready") {
      this.lastPanel = null;
      this.lastPanelPose = null;
      this.gazeDwell.reset();
      return Object.freeze({ timestampMs, headHud: menu.kind === "unavailable" ? NO_HEAD_HUD : resolveHeadHudFrame(viewModel.headHud, viewerPose),
        cameraPose: IDENTITY_POSE, viewport: this.viewport(), panel: menu.kind === "unavailable"
          ? Object.freeze({ kind: "unavailable", reason: menu.reason }) : Object.freeze({ kind: "absent" }) });
    }
    const headPose = presentationViewerPose ?? IDENTITY_POSE;
    let menuPose = this.menuPlacement.kind === "placed" ? this.menuPlacement.referenceFromMenu : placeMenuPanel(headPose, 2.4);
    if (panel !== null && (presentationViewerPose === null || viewer.source === "unavailable")) {
      this.lastPanel = null;
      this.lastPanelPose = null;
      this.gazeDwell.update(null, null, timestampMs);
      return Object.freeze({ timestampMs, headHud: NO_HEAD_HUD, cameraPose: IDENTITY_POSE,
        panel: Object.freeze({ kind: "unavailable", reason: viewer.source === "unavailable" ? viewer.reason : "viewer-unavailable" }), viewport: this.viewport() });
    }
    if (panel?.anchor === "menu" && presentationViewerPose !== null) {
      const placement = openMenuPlacement(this.menuPlacement, viewModel, panel, presentationViewerPose, viewer);
      this.menuPlacement = placement.model;
      if (placement.result.kind === "unavailable") {
        this.lastPanel = null;
        this.lastPanelPose = null;
        this.gazeDwell.update(null, null, timestampMs);
        return Object.freeze({ timestampMs, headHud: NO_HEAD_HUD, cameraPose: IDENTITY_POSE, panel: placement.result, viewport: this.viewport() });
      }
      menuPose = placement.result.referenceFromMenu;
    }
    const worldFromPanel = panel === null ? menuPose : resolveAnchorPose({ kind: panel.anchor, localPose: panel.localPose }, {
      world: this.referenceFromWorld,
      cockpit: this.referenceFromWorld,
      menu: menuPose,
      head: headPose
    });
    this.lastPanel = panel;
    this.lastPanelPose = worldFromPanel;
    const gazeCursor = this.updateGaze(panel, worldFromPanel, presentationViewerPose, timestampMs, menu);
    return Object.freeze({
      timestampMs,
      headHud: resolveHeadHudFrame(viewModel.headHud, viewerPose),
      cameraPose: IDENTITY_POSE,
      panel: panel === null ? Object.freeze({ kind: "absent" }) : Object.freeze({ kind: "visible", panel, pose: worldFromPanel, cursor: gazeCursor }),
      viewport: this.viewport()
    });
  }

  recenterMenu(): void {
    if (this.lastViewerPose !== null) this.menuPlacement = recenterMenuPlacement(this.menuPlacement, this.lastViewerPose);
  }

  private updateGaze(
    panel: UiViewModel["panels"][number] | null,
    worldFromPanel: Pose,
    viewerPose: Pose | null,
    timestampMs: number,
    menu: MenuPresentation
  ) {
    if (panel === null || viewerPose === null) return this.gazeDwell.update(null, null, timestampMs);
    const point = intersectPanel({
      origin: viewerPose.position,
      direction: rotateVec3(viewerPose.orientation, vec3(0, 0, -1))
    }, worldFromPanel);
    return menu.kind === "ready" ? this.gazeDwell.updateMenu(menu, point, timestampMs) : this.gazeDwell.update(panel, point, timestampMs);
  }

  private handleSelectRay(ray: SelectRay): void {
    const panel = this.lastPanel;
    const panelPose = this.lastPanelPose;
    if (panel === null || panelPose === null) return;
    const point = intersectPanel(ray, panelPose);
    if (point === null) return;
    const menu = this.lastMenu;
    if (menu.kind === "ready") {
      const identity = menuHitIdentity(menu, point);
      if (this.gazeDwell.wasActivatedRecently(identity, ray)) return;
      const action = menuActionAt(menu, point);
      if (action.kind === "action") {
        this.dispatch(menuFocusAction(menu, point));
        this.dispatch(action.action);
      }
      return;
    }
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
    this.menuPlacement = transformMenuPlacement(this.menuPlacement, transform);
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
