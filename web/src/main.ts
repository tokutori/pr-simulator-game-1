import "./styles.css";
import { createBootViewModel } from "./app/boot-view.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserPanelContext } from "./presentation/browser-canvas.js";
import { drawVrPanel, VR_PANEL_PIXELS } from "./presentation/vr-panel-canvas.js";
import { PresentationRuntime } from "./presentation/runtime.js";
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
const screenUi = new ScreenUiAdapter(uiRoot, showUnsupportedAction);
const panelCanvas = document.createElement("canvas");
panelCanvas.width = VR_PANEL_PIXELS.width;
panelCanvas.height = VR_PANEL_PIXELS.height;
let runtime: PresentationRuntime | null = null;
let resizeListener: (() => void) | null = null;
let pageHideListener: (() => void) | null = null;

renderViewModel(viewModel);
await initializeRenderer();

async function initializeRenderer(): Promise<void> {
  try {
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    const renderer = createThreeRenderer(canvas, panelCanvas);
    runtime = new PresentationRuntime(renderer, [new ScreenPresentationBackend(currentViewport)], () => viewModel);
    const started = await runtime.start("screen");
    if (!started.ok) {
      showStatus(`Renderer initialization failed: ${started.error.type}`);
      await runtime.dispose();
      runtime = null;
      return;
    }
    resizeListener = () => runtime?.resize(currentViewport());
    pageHideListener = () => {
      window.removeEventListener("resize", resizeListener as EventListener);
      void runtime?.dispose();
      screenUi.clear();
    };
    window.addEventListener("resize", resizeListener);
    window.addEventListener("pagehide", pageHideListener, { once: true });
    runtime.resize(currentViewport());
    showStatus("Screen presentation ready; flight systems are scheduled for later BPGs");
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
  renderViewModel(createBootViewModel(status));
}

function showUnsupportedAction(action: UiAction): void {
  showStatus(`Action ${action.type} is unavailable in Boot`);
}

function currentViewport(): ViewportSize {
  const ratio = Number.isFinite(window.devicePixelRatio) ? Math.min(window.devicePixelRatio, 2) : 1;
  return Object.freeze({ x: Math.max(1, window.innerWidth), y: Math.max(1, window.innerHeight), pixelRatio: ratio });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
