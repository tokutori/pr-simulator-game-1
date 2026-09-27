import "./styles.css";
import { createBootViewModel } from "./app/boot-view.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserPanelContext } from "./presentation/browser-canvas.js";
import { drawVrPanel, VR_PANEL_PIXELS } from "./presentation/vr-panel-canvas.js";
import { PresentationRuntime } from "./presentation/runtime.js";
import { WebXrPresentationBackend } from "./presentation/webxr-backend.js";
import { PhoneVrPresentationBackend } from "./presentation/phone-vr-backend.js";
import { createBrowserPhoneVrSensorPort } from "./presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "./presentation/phone-vr-gamepad-browser.js";
import type { UiAction, UiViewModel } from "./render/contracts/ui.js";
import type { ViewportSize } from "./render/contracts/runtime.js";

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

let viewModel: UiViewModel = createBootViewModel("Screen renderer is initializing");
const screenUi = new ScreenUiAdapter(uiRoot, handleUiAction);
const panelCanvas = document.createElement("canvas");
panelCanvas.width = VR_PANEL_PIXELS.width;
panelCanvas.height = VR_PANEL_PIXELS.height;
let runtime: PresentationRuntime | null = null;
let webXrBackend: WebXrPresentationBackend | null = null;
let phoneVrBackend: PhoneVrPresentationBackend | null = null;
let webXrAvailable = false;
let webXrActive = false;
let phoneVrAvailable = false;
let phoneVrActive = false;
let pageHidden = false;
let resizeListener: (() => void) | null = null;
let pageHideListener: (() => void) | null = null;

renderViewModel(viewModel);
await initializeRenderer();

async function initializeRenderer(): Promise<void> {
  try {
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    const bundle = createThreeRenderer(canvas, panelCanvas, navigator.xr ?? null);
    const screenBackend = new ScreenPresentationBackend(currentViewport);
    webXrBackend = new WebXrPresentationBackend(
      bundle.webxr,
      bundle.renderer,
      currentViewport,
      handleUiAction,
      onWebXrSessionEnd,
      onUnresolvableReferenceSpaceReset
    );
    phoneVrBackend = new PhoneVrPresentationBackend(
      createBrowserPhoneVrSensorPort(),
      bundle.renderer,
      currentViewport,
      handleUiAction,
      onPhoneVrTrackingUnavailable,
      { gamepadInput: createBrowserPhoneVrGamepadInputPort() }
    );
    runtime = new PresentationRuntime(bundle.renderer, [screenBackend, webXrBackend, phoneVrBackend], () => viewModel);
    const started = await runtime.start("screen");
    if (!started.ok) {
      showStatus(`Renderer initialization failed: ${started.error.type}`);
      await runtime.dispose();
      runtime = null;
      return;
    }
    resizeListener = () => runtime?.resize(currentViewport());
    pageHideListener = () => {
      pageHidden = true;
      window.removeEventListener("resize", resizeListener as EventListener);
      void webXrBackend?.cancelPendingRequest();
      void phoneVrBackend?.cancelPendingRequest();
      void runtime?.dispose();
      screenUi.clear();
    };
    window.addEventListener("resize", resizeListener);
    window.addEventListener("pagehide", pageHideListener, { once: true });
    runtime.resize(currentViewport());
    const [webXrAvailability, phoneVrAvailability] = await Promise.all([
      webXrBackend.checkAvailability(),
      phoneVrBackend.checkAvailability()
    ]);
    webXrAvailable = webXrAvailability.supported;
    phoneVrAvailable = phoneVrAvailability.supported;
    showStatus(`${webXrAvailability.message}; ${phoneVrAvailability.message}`);
  } catch (error) {
    await runtime?.dispose();
    runtime = null;
    showStatus(`3D rendering unavailable; Screen UI remains active: ${errorMessage(error)}`);
  }
}

function renderViewModel(next: UiViewModel): void {
  viewModel = next;
  screenUi.render(next);
  const panel = next.panels[0];
  const context = panelCanvas.getContext("2d");
  if (panel !== undefined && context !== null) {
    drawVrPanel(browserPanelContext(context), panel, VR_PANEL_PIXELS.width, VR_PANEL_PIXELS.height);
  }
}

function showStatus(status: string): void {
  renderViewModel(createBootViewModel(status, webXrAvailable, webXrActive, phoneVrAvailable, phoneVrActive));
}

function handleUiAction(action: UiAction): void {
  if (action.type === "focus" || action.type === "back" || action.type === "scroll") return;
  if (action.type === "activate" && action.controlId === "boot-enter-webxr") {
    enterWebXrFromUserGesture();
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-enter-phone-vr") {
    enterPhoneVrFromUserGesture();
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-exit-vr") {
    exitVrSession();
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-recenter-phone-tracking") {
    phoneVrBackend?.recenterTracking();
    showStatus("Phone VR tracking reference updated");
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-recenter-menu") {
    webXrBackend?.recenterMenu();
    phoneVrBackend?.recenterMenu();
    showStatus("Menu placement updated");
    return;
  }
  if (action.type === "recenter-menu") {
    webXrBackend?.recenterMenu();
    phoneVrBackend?.recenterMenu();
    return;
  }
  showStatus(`Action ${action.type} is unavailable in Boot`);
}

function enterPhoneVrFromUserGesture(): void {
  const backend = phoneVrBackend;
  const presentation = runtime;
  if (backend === null || presentation === null || !phoneVrAvailable) return;
  const permission = backend.requestPermissionFromUserGesture();
  showStatus("Waiting for phone orientation permission");
  void permission.then(async (result) => {
    if (pageHidden) return;
    if (!result.ok) {
      showStatus(result.message);
      return;
    }
    const started = await presentation.switchTo("phone-vr");
    if (!started.ok) {
      showStatus(`Phone VR initialization failed: ${started.error.type}`);
      return;
    }
    phoneVrActive = true;
    showStatus("Phone VR active; use head-gaze or standard Gamepad. Optical profile is unverified.");
  });
}

function enterWebXrFromUserGesture(): void {
  const backend = webXrBackend;
  const presentation = runtime;
  if (backend === null || presentation === null || !webXrAvailable) return;
  const request = backend.requestSessionFromUserGesture();
  showStatus("Waiting for WebXR permission");
  void request.then(async (result) => {
    if (pageHidden) return;
    if (!result.ok) {
      showStatus(result.message);
      return;
    }
    const started = await presentation.switchTo("webxr");
    if (!started.ok) {
      showStatus(`WebXR initialization failed: ${started.error.type}`);
      return;
    }
    webXrActive = true;
    showStatus("WebXR active; gaze dwell or XR select activates controls");
  });
}

function exitVrSession(): void {
  const presentation = runtime;
  if (presentation === null) return;
  showStatus("Ending VR session");
  void presentation.switchTo("screen").then((result) => {
    if (result.ok) {
      webXrActive = false;
      phoneVrActive = false;
    }
    showStatus(result.ok ? "VR ended; Screen is active" : `VR exit failed: ${result.error.type}`);
  });
}

function onWebXrSessionEnd(): void {
  webXrActive = false;
  showStatus("WebXR session ended; Screen remains active");
  void runtime?.switchTo("screen");
}

function onUnresolvableReferenceSpaceReset(): void {
  webXrActive = false;
  showStatus("WebXR tracking origin changed; return to Screen and restart the session");
  void runtime?.switchTo("screen");
}

function onPhoneVrTrackingUnavailable(message: string): void {
  phoneVrActive = false;
  const presentation = runtime;
  if (presentation === null) {
    showStatus(`${message}; Phone VR stopped`);
    return;
  }
  showStatus(`${message}; restoring Screen`);
  void presentation.switchTo("screen").then((result) => {
    if (pageHidden) return;
    showStatus(result.ok ? `${message}; Screen restored` : `${message}; Screen restore failed: ${result.error.type}`);
  });
}

function currentViewport(): ViewportSize {
  const ratio = Number.isFinite(window.devicePixelRatio) ? Math.min(window.devicePixelRatio, 2) : 1;
  return Object.freeze({ x: Math.max(1, window.innerWidth), y: Math.max(1, window.innerHeight), pixelRatio: ratio });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
