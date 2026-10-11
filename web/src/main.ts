import "./styles.css";
import { installBrowserPageLifecycle } from "./app/browser-page-lifecycle.js";
import { BrowserFlightLogDownload } from "./app/browser-flight-log-download.js";
import { createBootViewModel } from "./app/boot-view.js";
import { createGameViewModel } from "./app/game-view.js";
import { createFlightFrameViewDraft, createFlightUiHudModel, finalizeFlightFrameView, menuFrameFailureRecovery } from "./app/flight-frame-view.js";
import { NO_ENVIRONMENT_BRIEFING, parseEnvironmentBriefingSnapshot } from "./game/environment-briefing.js";
import { NO_HEAD_HUD_VIEW } from "./presentation/head-hud-view.js";
import { FlightControllerUiBindings } from "./app/flight-controller-port.js";
import { screenUiVisible } from "./app/presentation-visibility.js";
import { initializeAppSession } from "./app/session-factory.js";
import type { TailAppSessionFacade } from "./app/session-facade.js";
import { projectRuntimePlaybackClock, queryRuntimeRecordPose, readRuntimeSessionProjection } from "./app/session-runtime-projection.js";
import { createInitialAppModel, gameSessionPhaseCode, gameSessionSnapshot, isCurrentFlightLogDownload, isGameFlowActivation, isStaleGameFlowActivation, updateApp } from "./app/app-state.js";
import type {
  AppEffect,
  AppMessage,
  AppModel,
  FlightControllerIdentity,
  GameSessionOperation,
  ReplayClockCommand,
  ReplayClockState
} from "./app/app-state.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserHeadHudContext, drawHeadHud, headHudCanvasSize, prepareHeadHudPaint, validateHeadHudPaint } from "./presentation/head-hud-canvas.js";
import { browserMenuContext, drawMeasuredMenu, menuCanvasSize } from "./presentation/menu-canvas.js";
import { prepareMenuPresentation } from "./presentation/menu-preparation.js";
import type { MenuMeasurementCache } from "./presentation/menu-preparation.js";
import { menuContextCanInteract, menuExposesControl, menuInputGeometry, menuInputGeometryChanged } from "./presentation/menu-interaction.js";
import type { MenuInputGeometry } from "./presentation/menu-interaction.js";
import { PresentationRuntime } from "./presentation/runtime.js";
import { ScreenFrameRateDisplay } from "./presentation/frame-rate.js";
import { WebXrPresentationBackend } from "./presentation/webxr-backend.js";
import { PhoneVrPresentationBackend } from "./presentation/phone-vr-backend.js";
import { createBrowserPhoneVrSensorPort } from "./presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "./presentation/phone-vr-gamepad-browser.js";
import { BrowserTailPilotInput, DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION } from "./game/browser-tail-input.js";
import { TailFlightController } from "./game/tail-flight-controller.js";
import { suspendPageFlight } from "./game/page-flight-lifecycle.js";
import { DEFAULT_LAKE_VISUAL_CONDITION } from "./render/contracts/lake-water.js";
import { createArchivedPersonalBestSelection } from "./game/archived-personal-best.js";
import { FlightRecordRepository, IndexedDbFlightRecordPersistence } from "./game/flight-record-store.js";
import { NO_VENUE_MAP, venueMapForEnvironment } from "./game/biwa-venue-map.js";
import { analysisCursorMatches, projectAnalysisView } from "./game/flight-analysis-view.js";
import { parseRuntimeEnvironmentSnapshot, sameEnvironmentIdentity } from "./game/runtime-environment.js";
import type { RuntimeEnvironmentProjection } from "./game/runtime-environment.js";
import { projectRuntimeVenue } from "./game/runtime-venue.js";
import type { FlightDisplaySnapshot } from "./game/flight-display-snapshot.js";
import { projectRecordedFlightSnapshot } from "./game/flight-display-snapshot.js";
import { viewExposesAction } from "./render/contracts/ui.js";
import { FlightHudAdapter } from "./presentation/flight-hud.js";
import { resolveAttractCameraMode, resolveReplayCameraMode } from "./render/camera/camera-director.js";
import { cinematicCameraView, isCinematicCameraMode } from "./render/camera/cinematic-camera.js";
import type { FlightSnapshotInput } from "./app/session-snapshot.js";
import type { MenuScrollScope, UiAction } from "./render/contracts/ui.js";
import type { MenuPresentation, PreparedPresentationView, PresentationMode, RendererAdapter, RuntimeResult, ViewportSize } from "./render/contracts/runtime.js";
import type { ViewerFrame } from "./render/contracts/viewer-frame.js";
import type { HeadHudView } from "./presentation/head-hud-view.js";

const mount = document.getElementById("app");
if (!(mount instanceof HTMLElement)) throw new Error("Required app element is missing");
const displayLocale = document.documentElement.lang || "ja";

const stage = document.createElement("div");
stage.className = "presentation-shell";
const canvas = document.createElement("canvas");
canvas.className = "presentation-canvas";
canvas.setAttribute("aria-hidden", "true");
const uiRoot = document.createElement("div");
uiRoot.className = "screen-ui-root";
const flightHudRoot = document.createElement("section");
stage.append(canvas, uiRoot, flightHudRoot);
mount.replaceChildren(stage);
const frameRateDisplay = new ScreenFrameRateDisplay(stage);

const panelCanvas = document.createElement("canvas");
panelCanvas.width = 1536;
panelCanvas.height = 1152;
const headHudCanvas = document.createElement("canvas");
let model: AppModel = createInitialAppModel();
let runtime: PresentationRuntime | null = null;
let flightController: TailFlightController | null = null;
let gameSession: TailAppSessionFacade | null = null;
let physicsHz = 0;
let flightRenderer: RendererAdapter | null = null;
let countdownGeneration = 0;
let webXrBackend: WebXrPresentationBackend | null = null;
let phoneVrBackend: PhoneVrPresentationBackend | null = null;
const flightHud = new FlightHudAdapter(flightHudRoot);
const flightControllerUi = new FlightControllerUiBindings();
const controllerHudDisplay = Object.freeze({
  render: (snapshot: FlightSnapshotInput): void => {
    flightHud.renderDisplaySnapshot(snapshot, createFlightUiHudModel(model, snapshot));
  },
  setVisible: (visible: boolean): void => { flightHud.setVisible(visible); }
});
let preparedMenu: MenuPresentation = Object.freeze({ kind: "absent" });
let menuFontGeneration = 0;
let presentedMenuGeometry: MenuInputGeometry = Object.freeze({ kind: "none" });
const menuMeasurementCache: MenuMeasurementCache = new Map();
const screenUi = new ScreenUiAdapter(uiRoot, (action) => {
  dispatchUiAction(action);
});
const flightLogDownload = new BrowserFlightLogDownload(document);

function dispatch(message: AppMessage): void {
  const previousPhaseCode = gameSessionPhaseCode(model.gameSession);
  const transition = updateApp(model, message);
  model = transition.model;
  runSideEffects([
    () => {
      synchronizeMenuScroll();
      if ([9, 10].includes(previousPhaseCode) && ![9, 10].includes(gameSessionPhaseCode(model.gameSession))) flightController?.renderCurrentSnapshot();
      renderModel();
    },
    ...transition.effects.map((effect) => () => { runEffect(effect); })
  ]);
}

function runSideEffects(actions: readonly (() => void)[]): void {
  const failures: unknown[] = [];
  for (const action of actions) {
    try { action(); }
    catch (error: unknown) { failures.push(error); }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length > 1) throw new AggregateError(failures, failures.map(errorMessage).join("; "), { cause: failures[0] });
}

function renderModel(): void {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  const domVisible = screenUiVisible(model.presentation);
  frameRateDisplay.render(runtime?.framesPerSecond ?? null, domVisible);
  const environment = currentRuntimeEnvironment();
  flightRenderer?.setLakeVisualCondition(environment.kind === "available" ? environment.value.waves : DEFAULT_LAKE_VISUAL_CONDITION);
  flightRenderer?.setLakeSkyCondition(environment.kind === "available" && environment.value.sky.kind === "available"
    ? environment.value.sky.value.condition : null);
  flightRenderer?.setLakeVenueVisible(gameSession !== null && projectRuntimeVenue(environment, phaseCode).kind === "visible");
  flightHud.setInformationProfile(model.difficulty.informationCode, model.difficulty.hudProfile);
  const flightSnapshot = currentFlightSnapshot();
  if (flightSnapshot !== null && (phaseCode === 5 || phaseCode === 6)) controllerHudDisplay.render(flightSnapshot);
  flightHud.setVisible(domVisible && (phaseCode === 5 || phaseCode === 6));
  const presentationMode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
  const cameraMode = phaseCode === 10
    ? resolveAttractCameraMode(model.flightAnalysis, model.analysisCursorTimeSeconds, presentationMode)
    : phaseCode === 9
      ? resolveReplayCameraMode(model.replayCameraMode, model.flightAnalysis, model.analysisCursorTimeSeconds, presentationMode)
      : "pilot";
  const venue = currentVenueMap();
  const cameraPoints = venue.kind === "available" ? venue.value.cameraPoints : [];
  const cinematicView = isCinematicCameraMode(cameraMode) && model.replayPose !== null
    ? cinematicCameraView(cameraMode, model.replayPose, model.analysisCursorTimeSeconds, cameraPoints, model.flightAnalysis === null ? undefined : projectAnalysisView(model.flightAnalysis).samples)
    : null;
  flightRenderer?.setCinematicCameraView(cinematicView);
  flightRenderer?.setFlightCameraMode(cameraMode);
  if (phaseCode === 9 || phaseCode === 10) {
    flightRenderer?.setPreparedFlightPose(null);
    flightRenderer?.setFlightPose(model.replayPose);
  } else if (phaseCode === 2 || phaseCode === 3 || phaseCode === 4 || phaseCode === 8) {
    flightRenderer?.setFlightPose(null);
    flightRenderer?.setPreparedFlightPose(gameSession?.readPreparedLaunchPose() ?? null);
  } else {
    flightRenderer?.setPreparedFlightPose(null);
    if (phaseCode <= 1 || (phaseCode === 7 && flightController === null)) {
      flightRenderer?.setFlightPose(null);
    }
  }
  const viewModel = currentViewModel();
  screenUi.render(viewModel, domVisible);
}

function currentFrameViewModel(viewer: ViewerFrame): PreparedPresentationView {
  const frameModel = model;
  const snapshot = currentFlightSnapshot();
  frameRateDisplay.render(runtime?.framesPerSecond ?? null, runtime?.currentMode === "screen");
  const draft = createFlightFrameViewDraft(frameModel, snapshot, viewer, displayLocale, currentEnvironmentBriefing(), currentVenueMap(), runtime?.framesPerSecond ?? null);
  let headView: HeadHudView = draft.headHud;
  if (headView.kind === "visible") {
    const dimensions = headHudCanvasSize(headView.layer);
    if (headHudCanvas.width !== dimensions.width) headHudCanvas.width = dimensions.width;
    if (headHudCanvas.height !== dimensions.height) headHudCanvas.height = dimensions.height;
  }
  const headContext = headHudCanvas.getContext("2d");
  const headDrawingContext = headContext === null ? null : browserHeadHudContext(headContext);
  const preparation = headDrawingContext === null ? null : validateHeadHudPaint(
    prepareHeadHudPaint(headDrawingContext, headView, headHudCanvas.width, headHudCanvas.height), viewer);
  if (preparation !== null) {
    if (preparation.kind === "unavailable") headView = Object.freeze({ kind: "unavailable", reason: preparation.reason });
  } else if (headView.kind === "visible") {
    headView = Object.freeze({ kind: "unavailable", reason: "text-overflow" });
  }
  const viewModel = finalizeFlightFrameView(draft, headView);
  const panel = viewModel.panels[0];
  const panelContext = panelCanvas.getContext("2d");
  const scroll = frameModel.menuScroll;
  preparedMenu = panel === undefined || panel.anchor !== "menu" || scroll.kind !== "active" ? Object.freeze({ kind: "absent" })
    : document.fonts.status === "loading" ? Object.freeze({ kind: "pending" })
    : panelContext === null ? Object.freeze({ kind: "unavailable", reason: "context-unavailable" })
    : prepareMenuPresentation(browserMenuContext(panelContext), panel, { scope: scroll.scope, generation: scroll.generation }, scroll.progress,
      viewer, { locale: displayLocale, caption: viewModel.description, font: { family: "Arial, sans-serif", weight: 600, style: "normal", generation: menuFontGeneration } }, menuMeasurementCache);
  if (preparedMenu.kind === "ready") {
    if (menuInputGeometryChanged(presentedMenuGeometry, preparedMenu)) {
      const observedContext = preparedMenu.context;
      preparedMenu = Object.freeze({ kind: "pending" });
      queueMicrotask(() => { dispatch({ type: "menu-scroll-invalidated", context: observedContext }); });
    } else presentedMenuGeometry = menuInputGeometry(preparedMenu);
  } else if (preparedMenu.kind === "absent") presentedMenuGeometry = Object.freeze({ kind: "none" });
  if (headDrawingContext !== null && preparation !== null) drawHeadHud(headDrawingContext,
    viewModel.headHud.kind === "visible" ? preparation : { kind: "absent", width: preparation.width, height: preparation.height });
  if (preparedMenu.kind === "ready" && panelContext !== null) {
    const dimensions = menuCanvasSize(preparedMenu.panel.size);
    if (panelCanvas.width !== dimensions.width) panelCanvas.width = dimensions.width;
    if (panelCanvas.height !== dimensions.height) panelCanvas.height = dimensions.height;
    try {
      drawMeasuredMenu(browserMenuContext(panelContext), preparedMenu.viewport, panelCanvas.width, panelCanvas.height);
    } catch {
      preparedMenu = Object.freeze({ kind: "unavailable", reason: "drawing-unavailable" });
    }
  } else panelContext?.clearRect(0, 0, panelCanvas.width, panelCanvas.height);
  return Object.freeze({ viewModel, menu: preparedMenu });
}

function synchronizeMenuScroll(): void {
  const observed = model;
  const view = currentViewModel();
  const panel = view.panels.find((candidate) => candidate.anchor === "menu");
  const active = observed.presentation.type === "ready" && observed.presentation.mode !== "screen" && panel !== undefined;
  const viewKey = view.scene === "Result" ? `${observed.resultTab}:${observed.analysisChart}`
    : view.scene === "Replay" ? `${observed.replayViewMode}:${observed.analysisChart}` : "main";
  const scope: MenuScrollScope = view.activeOverlay === null
    ? { kind: "scene", scene: view.scene, panelId: panel?.id ?? "", viewKey }
    : { kind: "overlay", scene: view.scene, panelId: panel?.id ?? "", viewKey, overlay: view.activeOverlay };
  model = updateApp(model, { type: "menu-scroll-synchronized", generation: observed.menuScroll.generation,
    target: active ? { kind: "active", scope } : { kind: "closed" } }).model;
}

function invalidateMenuGeometry(): void {
  const scroll = model.menuScroll;
  if (scroll.kind === "active") dispatch({ type: "menu-scroll-invalidated", context: { scope: scroll.scope, generation: scroll.generation } });
  preparedMenu = Object.freeze({ kind: "pending" });
}

for (const fontEvent of ["loading", "loadingdone", "loadingerror"]) document.fonts.addEventListener(fontEvent, () => {
  menuFontGeneration++;
  menuMeasurementCache.clear();
  invalidateMenuGeometry();
});

function runEffect(effect: AppEffect): void {
  switch (effect.type) {
    case "apply-lake-water-quality": {
      const renderer = flightRenderer;
      try {
        if (renderer === null) throw new Error("水面rendererを利用できない");
        void renderer.setLakeWaterQuality(effect.quality).then((cleanup) => {
          if (flightRenderer === renderer) dispatch({ type: "lake-water-quality-applied", requestId: effect.requestId, quality: effect.quality, cleanup });
        }, (error: unknown) => {
          if (flightRenderer === renderer) dispatch({ type: "lake-water-quality-failed", requestId: effect.requestId, message: errorMessage(error) });
        });
      } catch (error: unknown) {
        dispatch({ type: "lake-water-quality-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "download-flight-log": {
      const session = gameSession;
      if (!isCurrentFlightLogDownload(model, effect.requestId, effect.source)) return;
      try {
        if (session === null || session.readLifecycle().phaseCode !== effect.source.phaseCode) throw new Error("有効なRust FlightRecordを取得できない");
        const token = session.captureQueryToken();
        const text = session.readFlightLog(effect.format);
        if (gameSession !== session || !isCurrentFlightLogDownload(model, effect.requestId, effect.source)) return;
        if (session.acceptQuery(token, text).kind === "stale") throw new Error("Rust FlightRecordの取得対象が変更された");
        flightLogDownload.download({
          text,
          format: effect.format,
          filename: `flight-log-${effect.source.phaseCode === 7 ? "result" : "replay"}-${String(effect.requestId)}.${effect.format}`
        });
      } catch (error: unknown) {
        if (gameSession === session) dispatch({ type: "flight-log-download-failed", requestId: effect.requestId, source: effect.source, message: errorMessage(error) });
        return;
      }
      if (gameSession === session) dispatch({ type: "flight-log-download-requested", requestId: effect.requestId, source: effect.source });
      return;
    }
    case "suspend-page-flight":
      suspendPageFlightState();
      return;
    case "restore-page-flight":
      onResize();
      clearSessionPauseReason(1);
      return;
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
      pauseForPresentationTransition();
      flightController?.suspend();
      void presentation.switchTo(effect.mode).then((result) => {
        const current = model.presentation.type === "transitioning" && model.presentation.requestId === effect.requestId;
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, result);
        if (current) resolvePresentationTransitionPause();
      }, (error: unknown) => {
        const current = model.presentation.type === "transitioning" && model.presentation.requestId === effect.requestId;
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, {
          ok: false,
          error: { type: "renderer-failed", message: errorMessage(error) }
        });
        if (current) resolvePresentationTransitionPause();
      });
      return;
    }
    case "cancel-pending-request":
      if (effect.mode === "webxr") void webXrBackend?.cancelPendingRequest();
      else void phoneVrBackend?.cancelPendingRequest();
      return;
    case "recenter-tracking":
      phoneVrBackend?.recenterTracking();
      invalidateMenuGeometry();
      return;
    case "recenter-menu":
      if (runtime?.currentMode === "webxr") webXrBackend?.recenterMenu();
      if (runtime?.currentMode === "phone-vr") phoneVrBackend?.recenterMenu();
      invalidateMenuGeometry();
      return;
    case "dispose-presentation": {
      const presentation = runtime;
      runtime = null;
      flightRenderer = null;
      void presentation?.dispose();
      return;
    }
    case "game-session-operation":
      runGameSessionOperation(effect.operation, effect.requestId, gameSessionPhaseCode(model.gameSession));
      return;
    case "control-replay-clock":
      runReplayClockCommand(effect.requestId, effect.generation, effect.command);
      return;
    case "persist-flight-record":
      if (gameSession !== null) void persistFlightRecord(gameSession);
      return;
    case "load-stored-flight-records": {
      const repository = createFlightRecordRepository();
      void repository.list().then((records) => {
        dispatch({ type: "stored-flight-records-loaded", requestId: effect.requestId, records });
      }, (error: unknown) => {
        dispatch({ type: "stored-flight-records-failed", requestId: effect.requestId, message: errorMessage(error) });
      });
      return;
    }
    case "open-stored-flight-record": {
      const session = gameSession;
      if (session === null) {
        dispatch({ type: "game-operation-failed", requestId: effect.requestId, message: "Rust GameSession is unavailable" });
        return;
      }
      const token = session.captureQueryToken();
      void createFlightRecordRepository().load(effect.id).then((json) => {
        if (json === null) throw new Error(`FlightRecord ${String(effect.id)} does not exist`);
        if (gameSession !== session || session.acceptQuery(token, json).kind === "stale" || model.pendingGameRequestId !== effect.requestId) return;
        session.openArchive(json);
        completeGameOperation(effect.requestId);
      }).catch((error: unknown) => {
        if (gameSession !== session || model.pendingGameRequestId !== effect.requestId) return;
        dispatchGameOperationFailure(effect.requestId, errorMessage(error));
      });
      return;
    }
    case "load-flight-analysis": {
      const session = gameSession;
      if (session === null) {
        dispatch({ type: "flight-analysis-failed", requestId: effect.requestId, message: "Rust GameSession is unavailable" });
        return;
      }
      try {
        const token = session.captureQueryToken();
        const analysis = session.readAnalysisDataset({ kind: "available", value: {
          northMinimumMeters: -1_500, eastMinimumMeters: -1_500, altitudeMeters: 10, spacingMeters: 750
        } });
        if (gameSession !== session || session.acceptQuery(token, analysis).kind === "stale") return;
        dispatch({ type: "flight-analysis-loaded", requestId: effect.requestId, data: analysis });
      } catch (error: unknown) {
        dispatch({ type: "flight-analysis-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "load-flight-analysis-cursor": {
      const session = gameSession;
      const analysis = model.flightAnalysis;
      if (session === null || analysis === null) {
        dispatch({ type: "flight-analysis-cursor-failed", requestId: effect.requestId, message: "Rust GameSession is unavailable" });
        return;
      }
      try {
        const token = session.captureQueryToken();
        const sample = session.queryAnalysisCursor(effect.timeSeconds, analysis);
        if (gameSession !== session || session.acceptQuery(token, sample).kind === "stale") return;
        dispatch({ type: "flight-analysis-cursor-loaded", requestId: effect.requestId, sample });
      } catch (error: unknown) {
        dispatch({ type: "flight-analysis-cursor-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "load-flight-replay-pose": {
      const session = gameSession;
      const analysis = model.flightAnalysis;
      if (session === null || analysis === null) {
        dispatch({ type: "flight-replay-pose-failed", requestId: effect.requestId, message: "Rust FlightRecord is unavailable" });
        return;
      }
      try {
        const token = session.captureQueryToken();
        const { pose } = queryRuntimeRecordPose(session, analysis, effect.timeSeconds);
        if (gameSession !== session || session.acceptQuery(token, pose).kind === "stale") return;
        dispatch({ type: "flight-replay-pose-loaded", requestId: effect.requestId, pose });
      } catch (error: unknown) {
        dispatch({ type: "flight-replay-pose-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "schedule-replay-clock-tick": {
      const startedAt = performance.now();
      const session = gameSession;
      if (session === null) return;
      const token = session.captureQueryToken();
      window.setTimeout(() => {
        if (gameSession !== session || session.acceptQuery(token, effect).kind === "stale") return;
        dispatch({
          type: "replay-clock-tick",
          generation: effect.generation,
          elapsedSeconds: Math.max(0, performance.now() - startedAt) / 1000
        });
      }, effect.delayMilliseconds);
      return;
    }
    default:
      return assertNever(effect);
  }
}

function runReplayClockCommand(
  requestId: number,
  generation: number,
  command: ReplayClockCommand
): void {
  const session = gameSession;
  if (session === null) {
    dispatch({
      type: "replay-clock-command-failed",
      requestId,
      generation,
      message: "Rust GameSession is unavailable"
    });
    return;
  }
  try {
    switch (command.kind) {
      case "synchronize":
        if (command.seekTimeSeconds !== null) session.seekPlayback(command.seekTimeSeconds);
        break;
      case "play":
        session.setPlaybackPlaying(true);
        break;
      case "pause":
        session.setPlaybackPlaying(false);
        break;
      case "seek":
        session.seekPlayback(command.timeSeconds);
        break;
      case "rate":
        session.setPlaybackRate(command.rateCode);
        break;
      case "advance":
        session.advancePlayback(command.elapsedSeconds);
        break;
      default:
        return assertNever(command);
    }
    dispatch({
      type: "replay-clock-command-completed",
      requestId,
      generation,
      state: projectRuntimePlaybackClock(session.readPlaybackClock())
    });
  } catch (error: unknown) {
    let state: ReplayClockState | undefined;
    try {
      if (gameSession === session && [9, 10].includes(session.readLifecycle().phaseCode)) {
        session.setPlaybackPlaying(false);
        state = projectRuntimePlaybackClock(session.readPlaybackClock());
      }
    } catch {
      state = undefined;
    }
    dispatch({
      type: "replay-clock-command-failed",
      requestId,
      generation,
      message: errorMessage(error),
      ...(state === undefined ? {} : { state })
    });
  }
}

async function initializePresentation(requestId: number): Promise<void> {
  let presentationOwner:
    | { readonly kind: "uncreated" }
    | { readonly kind: "renderer"; readonly renderer: RendererAdapter }
    | { readonly kind: "runtime"; readonly runtime: PresentationRuntime } = { kind: "uncreated" };
  try {
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    if (model.presentation.type === "hidden") return;
    const bundle = createThreeRenderer(canvas, panelCanvas, navigator.xr ?? null, model.lakeWaterQuality.applied, headHudCanvas);
    presentationOwner = { kind: "renderer", renderer: bundle.renderer };
    const initializedSession = await initializeAppSession({ controlModeCode: 0, seedLow: 0x55aa, seedHigh: 0x5f98 });
    const session = initializedSession;
    gameSession = session;
    if (isPageHidden()) throw new Error("Page became hidden during initialization");
    physicsHz = initializedSession.physicsHz;
    flightRenderer = bundle.renderer;
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
    const presentation = new PresentationRuntime(
      bundle.renderer,
      [screenBackend, webXrBackend, phoneVrBackend],
        currentFrameViewModel,
        (timestampMs) => flightController?.onFrame(timestampMs),
        (mode, reason) => {
          const frameModel = model;
          queueMicrotask(() => {
            const message = menuFrameFailureRecovery(model, frameModel, mode, reason);
            if (message !== null) dispatch(message);
          });
        }
    );
    presentationOwner = { kind: "runtime", runtime: presentation };
    runtime = presentation;
    const started = await presentation.start("screen");
    if (!started.ok) throw new Error(`Renderer initialization failed: ${runtimeErrorMessage(started)}`);
    if (isPageHidden()) throw new Error("Page became hidden during initialization");
    window.addEventListener("resize", onResize);
    const [webXrAvailability, phoneVrAvailability] = await Promise.all([
      webXrBackend.checkAvailability(),
      phoneVrBackend.checkAvailability()
    ]);
    if (isPageHidden() || gameSession !== session) throw new Error("Page became hidden during initialization");
    dispatch({
      type: "presentation-initialized",
      requestId,
      activeMode: presentation.currentMode,
      webXrAvailable: webXrAvailability.supported,
      phoneVrAvailable: phoneVrAvailability.supported,
      status: `${webXrAvailability.message}; ${phoneVrAvailability.message}`
    });
    syncGameSession();
  } catch (error) {
    window.removeEventListener("resize", onResize);
    const ownedPresentation = presentationOwner;
    const controller = flightController;
    const session = gameSession;
    runtime = null;
    flightController = null;
    gameSession = null;
    flightRenderer = null;
    const cleanupFailures: string[] = [];
    const cleanups = [
      { name: "presentation", run: () => {
        switch (ownedPresentation.kind) {
          case "uncreated": return;
          case "renderer": ownedPresentation.renderer.dispose(); return;
          case "runtime": return ownedPresentation.runtime.dispose();
        }
      } },
      { name: "controller", run: () => controller?.dispose() },
      { name: "session", run: () => session?.dispose() }
    ];
    for (const cleanup of cleanups) {
      try {
        const result = await cleanup.run();
        if (result !== undefined && !result.ok) cleanupFailures.push(`${cleanup.name} cleanup failed: ${runtimeErrorMessage(result)}`);
      }
      catch (cleanupError: unknown) { cleanupFailures.push(`${cleanup.name} cleanup failed: ${errorMessage(cleanupError)}`); }
    }
    dispatch({
      type: "presentation-initialization-failed",
      requestId,
      message: `3D rendering unavailable; Screen UI remains active: ${[errorMessage(error), ...cleanupFailures].join("; ")}`
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

function currentViewModel() {
  if (gameSessionPhaseCode(model.gameSession) < 0) return createBootViewModel(model);
  return createGameViewModel(
    model,
    currentFlightSnapshot(),
    model.flightAnalysis,
    NO_HEAD_HUD_VIEW,
    currentEnvironmentBriefing(),
    currentVenueMap()
  );
}

function currentEnvironmentBriefing() {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  let environment = NO_ENVIRONMENT_BRIEFING;
  if ([1, 2, 3, 8].includes(phaseCode) && gameSession !== null) {
    try {
      environment = parseEnvironmentBriefingSnapshot(gameSession.readEnvironmentJson(), phaseCode);
    } catch {
      environment = NO_ENVIRONMENT_BRIEFING;
    }
  }
  return environment;
}

function runGameSessionOperation(operation: GameSessionOperation, requestId: number, requestedPhaseCode: number): void {
  const session = gameSession;
  if (session === null) {
    dispatch({ type: "game-operation-failed", requestId, message: "Rust GameSession is unavailable" });
    return;
  }
  try {
    if (operation === "cancel-countdown") countdownGeneration += 1;
    const result = session.executeOperation(operation);
    if (result.kind === "countdown-started") {
      completeGameOperation(requestId);
      countdownGeneration += 1;
      scheduleCountdownTick(countdownGeneration);
      return;
    }
    if (operation === "pause") flightController?.suspend();
    if (operation === "resume") flightController?.resume();
    if (result.kind === "aborted") {
      runSideEffects([() => { completeGameOperation(requestId); }, () => { flightController?.suspend(); }]);
      return;
    }
    completeGameOperation(requestId);
  } catch (error: unknown) {
    dispatchGameOperationFailure(requestId, errorMessage(error), { operation, requestedPhaseCode });
  }
}

function dispatchGameOperationFailure(
  requestId: number,
  message: string,
  context?: { readonly operation: GameSessionOperation; readonly requestedPhaseCode: number }
): void {
  const session = gameSession;
  if (session === null) {
    dispatch({ type: "game-operation-failed", requestId, message });
    return;
  }
  try {
    const currentSession = readRuntimeSessionProjection(session);
    const phaseCode = currentSession.phaseCode;
    if (phaseCode === 7 && model.gameSession.kind === "result" && model.pendingGameRequestId !== requestId) {
      dispatch({ type: "game-session-status", message: `終端表示処理に失敗した: ${message}` });
      return;
    }
    const failureMessage = context === undefined
      ? message
      : `${gameOperationLabel(context.operation)} rejected: UI phase ${String(context.requestedPhaseCode)}, Rust phase ${String(phaseCode)}; ${message}`;
    dispatch({ type: "game-operation-failed", requestId, message: failureMessage, currentSession });
  } catch (syncError: unknown) {
    dispatch({
      type: "game-operation-failed",
      requestId,
      message: `${message}; GameSession resync failed: ${errorMessage(syncError)}`
    });
  }
}

function gameOperationLabel(operation: GameSessionOperation): string {
  return typeof operation === "string"
    ? operation
    : operation.kind === "set-information-cue"
      ? `set-information-cue-${String(operation.cueCode)}-${String(operation.visible)}`
      : `set-${operation.axis}-${String(operation.code)}`;
}

function completeGameOperation(requestId: number): void {
  const session = gameSession;
  if (session === null) return;
  dispatch({
    type: "game-operation-completed",
    requestId,
    ...readRuntimeSessionProjection(session)
  });
}

function scheduleCountdownTick(generation: number): void {
  const owner = gameSession;
  if (owner === null) return;
  const token = owner.captureQueryToken();
  window.setTimeout(() => {
    const session = gameSession;
    if (session !== owner || owner.acceptQuery(token, generation).kind === "stale" || generation !== countdownGeneration
        || owner.readLifecycle().phaseCode !== 4) return;
    try {
      const remainingTicks = owner.advanceCountdown();
      syncGameSession();
      if (remainingTicks === 0) {
        owner.launch();
        syncGameSession();
        if (flightController === null) createFlightController(owner);
        else resetFlightController();
      } else {
        scheduleCountdownTick(generation);
      }
    } catch (error: unknown) {
      syncGameSession();
      dispatch({ type: "game-session-status", message: `発進準備に失敗した: ${errorMessage(error)}` });
    }
  }, 800);
}

function createFlightController(session: TailAppSessionFacade): void {
  const renderer = flightRenderer;
  if (renderer === null) throw new Error("Flight renderer is unavailable");
  const binding = flightControllerUi.bindAppDisplay(session.flightPort, controllerHudDisplay,
    () => session.readGameSessionProjection(), () => controller.currentDisplaySnapshot, dispatch);
  const input = new BrowserTailPilotInput(window, { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION,
    keyboard: { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION.keyboard, physicsHz } });
  let controller: TailFlightController;
  try {
    controller = new TailFlightController(session.flightPort, input, renderer, binding.port, physicsHz, undefined, binding.onTerminal);
  } catch (error: unknown) {
    let failure = error;
    try { input.dispose(); }
    catch (cleanup: unknown) { failure = new AggregateError([error, cleanup], `${errorMessage(error)}; input cleanup failed: ${errorMessage(cleanup)}`, { cause: error }); }
    publishControllerInitializationFailure(session, binding.identity, failure);
    throw failure;
  }
  flightController = controller;
  dispatch({ type: "flight-controller-ready", identity: binding.identity });
}

function resetFlightController(): void {
  const controller = flightController;
  const session = gameSession;
  if (controller === null || session === null) throw new Error("Flight controller is unavailable");
  const binding = flightControllerUi.bindAppDisplay(session.flightPort, controllerHudDisplay,
    () => session.readGameSessionProjection(), () => controller.currentDisplaySnapshot, dispatch);
  try { controller.reset(session.flightPort.snapshot_json(), binding.port, binding.onTerminal); }
  catch (error: unknown) {
    publishControllerInitializationFailure(session, binding.identity, error);
    throw error;
  }
  dispatch({ type: "flight-controller-ready", identity: binding.identity });
}

function publishControllerInitializationFailure(session: TailAppSessionFacade,
  identity: FlightControllerIdentity, error: unknown): void {
  dispatch({ type: "flight-controller-ready", identity });
  const projection = session.readGameSessionProjection();
  if (projection.phaseCode === 5 || projection.phaseCode === 6) {
    dispatch({ type: "flight-controller-stopped", identity, message: errorMessage(error), snapshot: projection.display.value });
  }
}

async function persistFlightRecord(
  session: NonNullable<typeof gameSession>
): Promise<void> {
  const token = session.captureQueryToken();
  try {
    const repository = createFlightRecordRepository();
    const saved = await repository.saveFrom({ export_flight_record_json: () => session.exportRecordJson() });
    dispatch({ type: "refresh-stored-flight-records" });
    if (gameSession === session && session.acceptQuery(token, saved).kind === "accepted" && session.readLifecycle().phaseCode === 7) {
      const personalBestMessage = saved.personalBest.kind === "candidate"
        ? "Personal Bestを更新した"
        : saved.personalBest.kind === "existing"
          ? `Personal Best ${String(saved.personalBest.id)}を維持した`
          : saved.personalBest.kind === "ineligible"
            ? "Personal Best対象外の記録である"
            : "";
      dispatch({
        type: "game-session-status",
        message: `FlightRecord ${String(saved.id)}を保存した${personalBestMessage.length > 0 ? `。${personalBestMessage}` : ""}`
      });
    }
  } catch (error: unknown) {
    if (gameSession === session && session.acceptQuery(token, undefined).kind === "accepted" && session.readLifecycle().phaseCode === 7) {
      dispatch({ type: "game-session-status", message: `FlightRecord保存に失敗した: ${errorMessage(error)}` });
    }
  }
}

function createFlightRecordRepository(): FlightRecordRepository {
  return new FlightRecordRepository(
    new IndexedDbFlightRecordPersistence(window.indexedDB),
    undefined,
    createArchivedPersonalBestSelection
  );
}

function dispatchUiAction(action: UiAction): void {
  if (action.type === "menu-scroll" || action.type === "menu-focus" || action.type === "menu-control") {
    if (!menuContextCanInteract(preparedMenu, model.menuScroll, action.context) || preparedMenu.kind !== "ready") return;
    if (action.type === "menu-scroll") {
      dispatch({ type: "menu-scroll", context: action.context, intent: action.intent });
      return;
    }
    if (action.type === "menu-focus") {
      if (action.focus.kind === "control" && !menuExposesControl(preparedMenu, action.focus.controlId)) return;
      dispatch({ type: "menu-focus", context: action.context, focus: action.focus });
      return;
    }
    if (!menuExposesControl(preparedMenu, action.action.controlId)) return;
    dispatchUiAction(action.action);
    return;
  }
  const session = gameSession;
  const displayedPhaseCode = gameSessionPhaseCode(model.gameSession);
  if (session !== null && isGameFlowActivation(action)) {
    syncGameSession();
    const synchronizedPhaseCode = gameSessionPhaseCode(model.gameSession);
    if (isStaleGameFlowActivation(action, displayedPhaseCode, synchronizedPhaseCode)
        && !viewExposesAction(currentViewModel(), action)) return;
  }
  if ((action.type === "activate" || action.type === "set-toggle" || action.type === "set-range")
      && !viewExposesAction(currentViewModel(), action)) return;
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
  invalidateMenuGeometry();
  runtime?.resize(currentViewport());
}

function onPageHide(): void {
  flightLogDownload.dispose();
  window.removeEventListener("resize", onResize);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  const controller = flightController;
  const session = gameSession;
  flightController = null;
  gameSession = null;
  try {
    controller?.dispose();
  } finally {
    try { session?.dispose(); }
    finally { dispatch({ type: "page-hidden" }); }
  }
}

function onVisibilityChange(): void {
  if (document.visibilityState === "hidden") {
    suspendPageFlightState();
    return;
  }
  if (model.presentation.type === "cached") {
    onPageRestore();
    return;
  }
  clearSessionPauseReason(1);
}

function suspendPageFlightState(): void {
  try {
    const session = gameSession;
    const pagePort = session === null ? null : {
      phase_code: () => session.readLifecycle().phaseCode,
      cancel_countdown: () => { session.executeOperation("cancel-countdown"); },
      pause: (reason: number) => {
        if (reason !== 1) throw new RangeError("Page suspension requires the page-hidden reason");
        session.pauseForReason("page_hidden");
      }
    };
    suspendPageFlight(pagePort, flightController, () => { countdownGeneration += 1; }, syncGameSession);
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `非表示化に伴う停止に失敗した: ${errorMessage(error)}` });
  }
}

function onPageSuspend(): void {
  dispatch({ type: "page-suspended" });
}

function onPageRestore(): void {
  if (document.visibilityState === "hidden") return;
  dispatch({ type: "page-restored" });
}

function pauseForPresentationTransition(): void {
  pauseSessionForReason("presentation_transition");
}

function resolvePresentationTransitionPause(): void {
  clearSessionPauseReason(2);
}

function pauseSessionForReason(reason: "page_hidden" | "presentation_transition"): void {
  const session = gameSession;
  if (session === null || ![5, 6].includes(session.readLifecycle().phaseCode)) return;
  try {
    session.pauseForReason(reason);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `飛行の一時停止に失敗した: ${errorMessage(error)}` });
  }
}

function clearSessionPauseReason(reason: number): void {
  const session = gameSession;
  if (session === null || session.readLifecycle().phaseCode !== 6) return;
  try {
    session.clearPauseReason(reason);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `一時停止状態の更新に失敗した: ${errorMessage(error)}` });
  }
}

function syncGameSession(): void {
  const session = gameSession;
  if (session === null) return;
  dispatch({ type: "game-session-synced", ...readRuntimeSessionProjection(session) });
}

function currentFlightSnapshot(): FlightDisplaySnapshot | null {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  const cursor = model.analysisCursorSample;
  const analysis = model.flightAnalysis;
  if ((phaseCode === 9 || phaseCode === 10) && cursor !== null && "kind" in cursor && analysis !== null
      && analysisCursorMatches(analysis, cursor)) return projectRecordedFlightSnapshot(cursor, cursor.context);
  if (model.flightExecution.kind === "stopped" && (phaseCode === 5 || phaseCode === 6)) return model.flightExecution.snapshot;
  if (phaseCode === 5 && flightController !== null && model.flightExecution.kind === "ready") {
    const current = flightController.currentDisplaySnapshot;
    if (current.kind === "tail_flight" && current.phaseCode === phaseCode) return current;
  }
  return gameSessionSnapshot(model.gameSession);
}

function currentRuntimeEnvironment(): RuntimeEnvironmentProjection {
  const session = gameSession;
  if (session === null) return Object.freeze({ kind: "unavailable", reason: "no_selection" });
  try {
    const phaseCode = session.readLifecycle().phaseCode;
    const identity = phaseCode === 9 || phaseCode === 10 ? session.readPlaybackContext().scenario
      : session.readSnapshot().identity;
    const expected = "kind" in identity ? identity.kind === "prepared" ? identity.scenario : undefined : identity;
    const environment = parseRuntimeEnvironmentSnapshot(session.readEnvironmentJson(), phaseCode, expected);
    const analysis = model.flightAnalysis;
    if (analysis !== null && environment.kind === "available"
        && !sameEnvironmentIdentity(environment.value.identity, analysis.context.scenario)) return Object.freeze({ kind: "unavailable", reason: "invalid_snapshot" });
    return environment;
  } catch {
    return Object.freeze({ kind: "unavailable", reason: "invalid_snapshot" });
  }
}

function currentVenueMap() {
  const analysis = model.flightAnalysis;
  const environment = currentRuntimeEnvironment();
  if (analysis !== null && environment.kind === "available"
      && !sameEnvironmentIdentity(environment.value.identity, analysis.context.scenario)) return NO_VENUE_MAP;
  return venueMapForEnvironment(environment);
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
    case "phone-vr": return "Phone VR active with gravity-referenced tilt; recenter resets heading only. Device and optical validation remain pending.";
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
installBrowserPageLifecycle(window, { suspend: onPageSuspend, restore: onPageRestore, dispose: onPageHide });
document.addEventListener("visibilitychange", onVisibilityChange);
dispatch({ type: "initialize" });
