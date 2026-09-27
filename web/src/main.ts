import "./styles.css";
import { createBootViewModel } from "./app/boot-view.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserPanelContext } from "./presentation/browser-canvas.js";
import { drawVrPanel, VR_PANEL_PIXELS } from "./presentation/vr-panel-canvas.js";
import { PresentationRuntime } from "./presentation/runtime.js";
import { WebXrPresentationBackend } from "./presentation/webxr-backend.js";
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
let webXrAvailable = false;
let webXrActive = false;
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
    runtime = new PresentationRuntime(bundle.renderer, [screenBackend, webXrBackend], () => viewModel);
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
      void runtime?.dispose();
      screenUi.clear();
    };
    window.addEventListener("resize", resizeListener);
    window.addEventListener("pagehide", pageHideListener, { once: true });
    runtime.resize(currentViewport());
    const availability = await webXrBackend.checkAvailability();
    webXrAvailable = availability.supported;
    showStatus(availability.message);
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
  renderViewModel(createBootViewModel(status, webXrAvailable, webXrActive));
}

function handleUiAction(action: UiAction): void {
  if (action.type === "focus") return;
  if (action.type === "activate" && action.controlId === "boot-enter-webxr") {
    enterWebXrFromUserGesture();
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-exit-webxr") {
    exitWebXr();
    return;
  }
  if (action.type === "activate" && action.controlId === "boot-recenter-menu") {
    webXrBackend?.recenterMenu();
    showStatus("Menu placement updated");
    return;
  }
  if (action.type === "recenter-menu") {
    webXrBackend?.recenterMenu();
    return;
  }
  showStatus(`Action ${action.type} is unavailable in Boot`);
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

function exitWebXr(): void {
  const presentation = runtime;
  if (presentation === null) return;
  showStatus("Ending WebXR session");
  void presentation.switchTo("screen").then((result) => {
    webXrActive = false;
    showStatus(result.ok ? "WebXR ended; Screen is active" : `WebXR exit failed: ${result.error.type}`);
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

function currentViewport(): ViewportSize {
  const ratio = Number.isFinite(window.devicePixelRatio) ? Math.min(window.devicePixelRatio, 2) : 1;
  return Object.freeze({ x: Math.max(1, window.innerWidth), y: Math.max(1, window.innerHeight), pixelRatio: ratio });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
