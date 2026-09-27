import "./styles.css";
import { createBootViewModel } from "./app/boot-view.js";
import { createInitialAppModel, updateApp } from "./app/app-state.js";
import type { AppEffect, AppMessage, AppModel } from "./app/app-state.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserPanelContext } from "./presentation/browser-canvas.js";
import { drawVrPanel, VR_PANEL_PIXELS } from "./presentation/vr-panel-canvas.js";
import { PresentationRuntime } from "./presentation/runtime.js";
import { WebXrPresentationBackend } from "./presentation/webxr-backend.js";
import { PhoneVrPresentationBackend } from "./presentation/phone-vr-backend.js";
import { createBrowserPhoneVrSensorPort } from "./presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "./presentation/phone-vr-gamepad-browser.js";
import type { UiAction } from "./render/contracts/ui.js";
import type { PresentationMode, RendererAdapter, RuntimeResult, ViewportSize } from "./render/contracts/runtime.js";

const mount = document.getElementById("app");
if (!(mount instanceof HTMLElement)) throw new Error("Required app element is missing");

const stage = document.createElement("div");
stage.className = "presentation-shell";
const canvas = document.createElement("canvas");
canvas.className = "presentation-canvas";
canvas.setAttribute("aria-hidden", "true");
const uiRoot = document.createElement("div");
uiRoot.className = "screen-ui-root";
stage.append(canvas, uiRoot);
mount.replaceChildren(stage);

const panelCanvas = document.createElement("canvas");
panelCanvas.width = VR_PANEL_PIXELS.width;
panelCanvas.height = VR_PANEL_PIXELS.height;
let model: AppModel = createInitialAppModel();
let runtime: PresentationRuntime | null = null;
let webXrBackend: WebXrPresentationBackend | null = null;
let phoneVrBackend: PhoneVrPresentationBackend | null = null;
const screenUi = new ScreenUiAdapter(uiRoot, (action) => {
  dispatch({ type: "ui-action", action });
});

function dispatch(message: AppMessage): void {
  const transition = updateApp(model, message);
  model = transition.model;
  renderModel();
  for (const effect of transition.effects) runEffect(effect);
}

function renderModel(): void {
  if (model.presentation.type === "hidden") {
    screenUi.clear();
    return;
  }
  const viewModel = createBootViewModel(model);
  screenUi.render(viewModel);
  const panel = viewModel.panels[0];
  const context = panelCanvas.getContext("2d");
  if (panel !== undefined && context !== null) {
    drawVrPanel(browserPanelContext(context), panel, VR_PANEL_PIXELS.width, VR_PANEL_PIXELS.height);
  }
}

function runEffect(effect: AppEffect): void {
  switch (effect.type) {
    case "initialize-presentation":
      void initializePresentation(effect.requestId);
      return;
    case "request-permission": {
      const request = effect.mode === "webxr"
        ? webXrBackend?.requestSessionFromUserGesture()
        : phoneVrBackend?.requestPermissionFromUserGesture();
      if (request === undefined) {
        dispatch({
          type: "permission-completed",
          requestId: effect.requestId,
          mode: effect.mode,
          ok: false,
          message: `${labelForMode(effect.mode)} permission service is unavailable`
        });
        return;
      }
      void request.then((result) => {
        dispatch({
          type: "permission-completed",
          requestId: effect.requestId,
          mode: effect.mode,
          ok: result.ok,
          message: result.ok ? "Permission granted" : result.message
        });
      }, (error: unknown) => {
        dispatch({
          type: "permission-completed",
          requestId: effect.requestId,
          mode: effect.mode,
          ok: false,
          message: errorMessage(error)
        });
      });
      return;
    }
    case "switch-backend": {
      const presentation = runtime;
      if (presentation === null) {
        dispatchBackendResult(effect.requestId, effect.mode, null, {
          ok: false,
          error: { type: "renderer-failed", message: "Presentation runtime is unavailable" }
        });
        return;
      }
      void presentation.switchTo(effect.mode).then((result) => {
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, result);
      }, (error: unknown) => {
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, {
          ok: false,
          error: { type: "renderer-failed", message: errorMessage(error) }
        });
      });
      return;
    }
    case "cancel-pending-request":
      if (effect.mode === "webxr") void webXrBackend?.cancelPendingRequest();
      else void phoneVrBackend?.cancelPendingRequest();
      return;
    case "recenter-tracking":
      phoneVrBackend?.recenterTracking();
      return;
    case "recenter-menu":
      if (runtime?.currentMode === "webxr") webXrBackend?.recenterMenu();
      if (runtime?.currentMode === "phone-vr") phoneVrBackend?.recenterMenu();
      return;
    case "dispose-presentation": {
      const presentation = runtime;
      runtime = null;
      void presentation?.dispose();
      return;
    }
    default:
      return assertNever(effect);
  }
}

async function initializePresentation(requestId: number): Promise<void> {
  let rendererAdapter: RendererAdapter | null = null;
  try {
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    if (model.presentation.type === "hidden") return;
    const bundle = createThreeRenderer(canvas, panelCanvas, navigator.xr ?? null);
    rendererAdapter = bundle.renderer;
    const screenBackend = new ScreenPresentationBackend(currentViewport);
    webXrBackend = new WebXrPresentationBackend(
      bundle.webxr,
      bundle.renderer,
      currentViewport,
      dispatchUiAction,
      onWebXrSessionEnd,
      onUnresolvableReferenceSpaceReset
    );
    phoneVrBackend = new PhoneVrPresentationBackend(
      createBrowserPhoneVrSensorPort(),
      bundle.renderer,
      currentViewport,
      dispatchUiAction,
      onPhoneVrTrackingUnavailable,
      { gamepadInput: createBrowserPhoneVrGamepadInputPort() }
    );
    const presentation = new PresentationRuntime(bundle.renderer, [screenBackend, webXrBackend, phoneVrBackend], () => createBootViewModel(model));
    runtime = presentation;
    const started = await presentation.start("screen");
    if (!started.ok) {
      const status = `Renderer initialization failed: ${runtimeErrorMessage(started)}`;
      await presentation.dispose();
      rendererAdapter = null;
      if (runtime === presentation) runtime = null;
      dispatch({ type: "presentation-initialization-failed", requestId, message: status });
      return;
    }
    if (isPageHidden()) {
      await presentation.dispose();
      rendererAdapter = null;
      if (runtime === presentation) runtime = null;
      return;
    }
    window.addEventListener("resize", onResize);
    const [webXrAvailability, phoneVrAvailability] = await Promise.all([
      webXrBackend.checkAvailability(),
      phoneVrBackend.checkAvailability()
    ]);
    dispatch({
      type: "presentation-initialized",
      requestId,
      activeMode: presentation.currentMode,
      webXrAvailable: webXrAvailability.supported,
      phoneVrAvailable: phoneVrAvailability.supported,
      status: `${webXrAvailability.message}; ${phoneVrAvailability.message}`
    });
  } catch (error) {
    window.removeEventListener("resize", onResize);
    const presentation = runtime;
    runtime = null;
    if (presentation !== null) {
      await presentation.dispose();
    } else {
      rendererAdapter?.dispose();
    }
    dispatch({
      type: "presentation-initialization-failed",
      requestId,
      message: `3D rendering unavailable; Screen UI remains active: ${errorMessage(error)}`
    });
  }
}

function dispatchBackendResult(
  requestId: number,
  requestedMode: PresentationMode,
  activeMode: PresentationMode | null,
  result: RuntimeResult
): void {
  dispatch({
    type: "backend-transition-completed",
    requestId,
    requestedMode,
    activeMode,
    ok: result.ok,
    message: result.ok ? "" : runtimeErrorMessage(result),
    successStatus: successStatus(requestedMode)
  });
}

function dispatchUiAction(action: UiAction): void {
  dispatch({ type: "ui-action", action });
}

function onWebXrSessionEnd(): void {
  dispatch({ type: "backend-ended", mode: "webxr", message: "WebXR session ended" });
}

function onUnresolvableReferenceSpaceReset(): void {
  dispatch({ type: "backend-ended", mode: "webxr", message: "WebXR tracking origin changed; restarting the session is required" });
}

function onPhoneVrTrackingUnavailable(message: string): void {
  dispatch({ type: "backend-ended", mode: "phone-vr", message: `${message}; restoring Screen` });
}

function onResize(): void {
  runtime?.resize(currentViewport());
}

function onPageHide(): void {
  window.removeEventListener("resize", onResize);
  dispatch({ type: "page-hidden" });
}

function currentViewport(): ViewportSize {
  const ratio = Number.isFinite(window.devicePixelRatio) ? Math.min(window.devicePixelRatio, 2) : 1;
  return Object.freeze({ x: Math.max(1, window.innerWidth), y: Math.max(1, window.innerHeight), pixelRatio: ratio });
}

function runtimeErrorMessage(result: Exclude<RuntimeResult, { readonly ok: true }>): string {
  if (result.error.type === "backend-failed") return `${result.error.type} (${result.error.mode}): ${result.error.message}`;
  if (result.error.type === "unsupported") return `${result.error.type}: ${result.error.mode}`;
  if (result.error.type === "renderer-failed") return `${result.error.type}: ${result.error.message}`;
  return result.error.type;
}

function isPageHidden(): boolean {
  return model.presentation.type === "hidden";
}

function successStatus(mode: PresentationMode): string {
  switch (mode) {
    case "screen": return "Screen is active";
    case "webxr": return "WebXR active; gaze dwell or XR select activates controls";
    case "phone-vr": return "Phone VR active; use head-gaze or standard Gamepad. Optical profile is unverified.";
  }
}

function labelForMode(mode: "webxr" | "phone-vr"): string {
  return mode === "webxr" ? "WebXR" : "Phone VR";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function assertNever(value: never): never {
  throw new Error(`Unhandled presentation effect: ${JSON.stringify(value)}`);
}

renderModel();
window.addEventListener("resize", onResize);
window.addEventListener("pagehide", onPageHide, { once: true });
dispatch({ type: "initialize" });
