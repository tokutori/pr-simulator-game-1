import type { GamePhase } from "./app/session-snapshot.js";
import "./styles.css";
import { installBrowserPageLifecycle } from "./app/browser-page-lifecycle.js";
import { BrowserFlightLogDownload } from "./app/browser-flight-log-download.js";
import { createBootViewModel } from "./app/boot-view.js";
import { createGameViewModel } from "./app/game-view.js";
import { createFlightFrameViewDraft, createFlightUiHudModel, finalizeFlightFrameView, menuFrameFailureRecovery } from "./app/flight-frame-view.js";
import { FlightControllerUiBindings } from "./app/flight-controller-port.js";
import { screenUiVisible } from "./app/presentation-visibility.js";
import { initializeAppSession } from "./app/session-factory.js";
import { initializeWasmRuntime } from "./game/wasm-flight.js";
import type { TailAppSessionFacade } from "./app/session-facade.js";
import { projectRuntimePlaybackClock, readRuntimeSessionProjection } from "./app/session-runtime-projection.js";
import { createInitialAppModel, flightAnalysisData, gameSessionPhase, isCurrentFlightLogDownload, isGameFlowActivation, isStaleGameFlowActivation, recordListEntries, recordListNotice, replayPoseData, updateApp } from "./app/app-state.js";
import type {
  AppEffect,
  AppMessage,
  AppModel,
  FlightControllerIdentity,
  ReplayClockCommand,
  ReplayClockState
} from "./app/app-state.js";
import type { SessionCommand } from "./app/game-ui-types.js";
import type { GameUiPlatformInput } from "./generated/game-ui-input.js";
import type { GameUiPlatformObservation } from "./app/game-ui-codec.js";
import { gameUiInputQuery } from "./generated/game-ui-input-query.js";
import type { GameUiUpdate } from "./generated/game-ui-effects.js";
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
import { maximum_flight_frame_gap_milliseconds, screen_vertical_field_of_view_degrees, ScreenRenderAutoBridge } from "../pkg/birdman_game_wasm.js";
import type { ScreenRenderAutoSnapshot } from "./generated/screen-render-auto.js";
import { suspendPageFlight } from "./game/page-flight-lifecycle.js";
import { DEFAULT_LAKE_VISUAL_CONDITION } from "./render/contracts/lake-water.js";
import { createArchivedPersonalBestSelection, personalBestComparisonVersion } from "./game/archived-personal-best.js";
import { FlightRecordRepository, IndexedDbFlightRecordPersistence } from "./game/flight-record-store.js";
import { projectRuntimeEnvironment } from "./game/runtime-environment.js";
import type { RuntimeEnvironmentProjection } from "./game/runtime-environment.js";
import { projectRuntimeVenue } from "./game/runtime-venue.js";
import { viewExposesAction } from "./render/contracts/ui.js";
import { FlightHudAdapter } from "./presentation/flight-hud.js";
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
let headHudResourceFailure: string | null = null;
let model: AppModel = createInitialAppModel();
let runtime: PresentationRuntime | null = null;
let flightController: TailFlightController | null = null;
let gameSession: TailAppSessionFacade | null = null;
let physicsHz = 0;
let flightRenderer: RendererAdapter | null = null;
let lastPresentationFrameMilliseconds: number | null = null;
let webXrBackend: WebXrPresentationBackend | null = null;
let phoneVrBackend: PhoneVrPresentationBackend | null = null;
const flightHud = new FlightHudAdapter(flightHudRoot);
const flightControllerUi = new FlightControllerUiBindings();
const controllerHudDisplay = Object.freeze({
  render: (): void => {
    synchronizeSharedGameUi();
    const hud = createFlightUiHudModel(model);
    flightHud.render(hud);
    flightHud.setVisible(screenUiVisible(model.presentation) && hud !== null);
  },
  setVisible: (visible: boolean): void => {
    flightHud.setVisible(visible && screenUiVisible(model.presentation) && model.sharedGameUi !== null && model.sharedGameUi.hud !== null);
  }
});
let preparedMenu: MenuPresentation = Object.freeze({ kind: "absent" });
let menuFontGeneration = 0;
let presentedMenuGeometry: MenuInputGeometry = Object.freeze({ kind: "none" });
const menuMeasurementCache: MenuMeasurementCache = new Map();
const screenUi = new ScreenUiAdapter(uiRoot, (action) => {
  dispatchUiAction(action);
}, () => {
  synchronizeSharedGameUi();
  synchronizeMenuScroll();
  renderModel();
});
const flightLogDownload = new BrowserFlightLogDownload(document);

function dispatch(message: AppMessage): void {
  const previousPhase = gameSessionPhase(model.gameSession);
  const transition = updateApp(model, message);
  model = transition.model;
  runSideEffects([
    () => {
      synchronizeSharedGameUi();
      synchronizeMenuScroll();
      if (["replay", "attract"].includes(previousPhase) && !["replay", "attract"].includes(gameSessionPhase(model.gameSession))) flightController?.renderCurrentSnapshot();
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
  const phase = gameSessionPhase(model.gameSession);
  const domVisible = screenUiVisible(model.presentation);
  frameRateDisplay.render(runtime?.framesPerSecond ?? null, domVisible);
  const environment = currentRuntimeEnvironment();
  flightRenderer?.setLakeVisualCondition(environment.kind === "available" ? environment.value.waves : DEFAULT_LAKE_VISUAL_CONDITION);
  flightRenderer?.setLakeSkyCondition(environment.kind === "available" && environment.value.sky.kind === "available"
    ? environment.value.sky.value.condition : null);
  flightRenderer?.setLakeVenueVisible(gameSession !== null && projectRuntimeVenue(environment, phase).kind === "visible");
  controllerHudDisplay.render();
  const replay = (phase === "replay" || phase === "attract") ? replayPoseData(model.replayPose) : null;
  const camera = replay?.replayCamera ?? null;
  flightRenderer?.setCinematicCameraView(camera?.cinematic ?? null);
  flightRenderer?.setFlightCameraMode(camera?.mode ?? "pilot");
  if (phase === "replay" || phase === "attract") {
    flightRenderer?.setPreparedFlightPose(null);
    flightRenderer?.setFlightPose(replay);
  } else if (phase === "briefing_preparing" || phase === "briefing_ready" || phase === "countdown" || phase === "briefing_failed") {
    flightRenderer?.setFlightPose(null);
    flightRenderer?.setPreparedFlightPose(gameSession?.readPreparedLaunchPose() ?? null);
  } else {
    flightRenderer?.setPreparedFlightPose(null);
    if (["boot", "title", "flight_setup"].includes(phase) || (phase === "result" && flightController === null)) {
      flightRenderer?.setFlightPose(null);
    }
  }
  const viewModel = currentViewModel();
  screenUi.render(viewModel, domVisible);
}

function currentFrameViewModel(viewer: ViewerFrame): PreparedPresentationView {
  synchronizeSharedGameUi();
  let frameModel = model;
  frameRateDisplay.render(runtime?.framesPerSecond ?? null, runtime?.currentMode === "screen");
  let draft = createFlightFrameViewDraft(frameModel, viewer, displayLocale, runtime?.framesPerSecond ?? null);
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
  const observedHeadHudFailure = headView.kind === "unavailable" ? headView.reason : null;
  if (headHudResourceFailure !== observedHeadHudFailure) {
    headHudResourceFailure = observedHeadHudFailure;
    synchronizeSharedGameUi();
    frameModel = model;
    draft = Object.freeze({ ...draft, model: frameModel });
  }
  const viewModel = finalizeFlightFrameView(draft, headView);
  const panel = viewModel.panels[0];
  const panelContext = panelCanvas.getContext("2d");
  const scroll = frameModel.menuScroll;
  const menuTheme = viewModel.sharedPresentation?.theme ?? frameModel.sharedGameUi?.theme;
  preparedMenu = panel === undefined || panel.anchor !== "menu" || scroll.kind !== "active" ? Object.freeze({ kind: "absent" })
    : menuTheme === undefined ? Object.freeze({ kind: "pending" })
    : document.fonts.status === "loading" ? Object.freeze({ kind: "pending" })
    : panelContext === null ? Object.freeze({ kind: "unavailable", reason: "context-unavailable" })
    : prepareMenuPresentation(browserMenuContext(panelContext), panel, { scope: scroll.scope, generation: scroll.generation }, scroll.progress,
      viewer, { theme: menuTheme, locale: displayLocale, caption: viewModel.description, font: { family: "Arial, sans-serif", weight: 600, style: "normal", generation: menuFontGeneration } }, menuMeasurementCache);
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
    case "select-screen-render-scale":
      flightRenderer?.setScreenRenderScale(Number(effect.scale) / 100);
      return;
    case "select-screen-render-auto":
      flightRenderer?.setScreenRenderAuto(effect.enabled);
      return;
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
        if (session === null || session.readLifecycle().phase !== effect.source.phase) throw new Error("有効なRust FlightRecordを取得できない");
        const text = session.readFlightLog(effect.format);
        if (gameSession !== session || !isCurrentFlightLogDownload(model, effect.requestId, effect.source)) return;
        flightLogDownload.download({
          text,
          format: effect.format,
          filename: `flight-log-${effect.source.phase === "result" ? "result" : "replay"}-${String(effect.requestId)}.${effect.format}`
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
      void presentation?.dispose().then((result) => {
        if (!result.ok) console.error("Presentation cleanup failed", result.error);
      }, (error: unknown) => { console.error("Presentation cleanup failed", error); });
      return;
    }
    case "update-shared-game-ui": {
      const session = gameSession;
      if (session === null) return;
      let result: GameUiUpdate;
      try {
        result = session.updateSharedUi(effect.action, sharedGameUiPlatformInput());
      } catch (error: unknown) {
        dispatch({ type: "game-session-status", message: `UI操作を適用できない: ${errorMessage(error)}` });
        return;
      }
      runSideEffects([
        () => {
          synchronizeSharedGameUi();
          dispatch({ type: "shared-game-ui-observed", view: session.readSharedUi(sharedGameUiPlatformInput()) });
        },
        ...result.effects.map((operation) => () => { dispatch({ type: "shared-game-ui-effect", effect: operation }); })
      ]);
      return;
    }
    case "shared-game-session-command":
      runSharedGameSessionCommand(effect.command, effect.requestId);
      return;
    case "shared-record-action": {
      try { gameSession?.executeSharedRecordAction(effect.action); }
      catch (error: unknown) { dispatch({ type: "game-session-status", message: `記録操作を適用できない: ${errorMessage(error)}` }); }
      return;
    }
    case "shared-replay-action": {
      const session = gameSession;
      if (session === null) return;
      try {
        session.executeSharedRecordAction(effect.action);
        dispatch({ type: "replay-clock-command-completed", requestId: effect.requestId, generation: effect.generation,
          state: projectRuntimePlaybackClock(session.readPlaybackClock()) });
      } catch (error: unknown) {
        dispatch({ type: "replay-clock-command-failed", requestId: effect.requestId, generation: effect.generation, message: errorMessage(error) });
      }
      return;
    }
    case "control-replay-clock":
      runReplayClockCommand(effect.requestId, effect.generation, effect.command);
      return;
    case "persist-flight-record":
      if (gameSession !== null) void persistFlightRecord(gameSession);
      return;
    case "load-stored-flight-records": {
      const repository = createFlightRecordRepository();
      void repository.recent(gameUiInputQuery.storedRecordLimit).then((records) => {
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
        const analysis = session.readAnalysisDataset({ kind: "available", value: {
          north_min_m: -1_500, east_min_m: -1_500, altitude_m: 10, spacing_m: 750
        } });
        dispatch({ type: "flight-analysis-loaded", requestId: effect.requestId, data: analysis });
      } catch (error: unknown) {
        dispatch({ type: "flight-analysis-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "load-flight-analysis-cursor": {
      const session = gameSession;
      const analysis = flightAnalysisData(model.analysis);
      if (session === null || analysis === null) {
        dispatch({ type: "flight-analysis-cursor-failed", requestId: effect.requestId, message: "Rust GameSession is unavailable" });
        return;
      }
      try {
        const sample = session.queryAnalysisCursor(effect.timeSeconds, analysis);
        dispatch({ type: "flight-analysis-cursor-loaded", requestId: effect.requestId, sample });
      } catch (error: unknown) {
        dispatch({ type: "flight-analysis-cursor-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "load-flight-replay-pose": {
      const session = gameSession;
      const analysis = flightAnalysisData(model.analysis);
      if (session === null || analysis === null) {
        dispatch({ type: "flight-replay-pose-failed", requestId: effect.requestId, message: "Rust FlightRecord is unavailable" });
        return;
      }
      try {
        const pose = session.queryReplayCamera(effect.timeSeconds, model.replayCameraMode,
          model.presentation.type === "ready" && model.presentation.mode === "screen", analysis);
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
      window.setTimeout(() => {
        if (gameSession !== session || model.replayClock.generation !== effect.generation) return;
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
      if (gameSession === session && ["replay", "attract"].includes(session.readLifecycle().phase)) {
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
    | { readonly kind: "policy"; readonly policy: ScreenRenderAutoBridge }
    | { readonly kind: "renderer"; readonly renderer: RendererAdapter }
    | { readonly kind: "runtime"; readonly runtime: PresentationRuntime } = { kind: "uncreated" };
  try {
    await initializeWasmRuntime();
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    if (model.presentation.type === "hidden") return;
    if (isPageHidden()) return;
    const retainedSelection = model.screenRenderScale?.selection;
    const policy = new ScreenRenderAutoBridge();
    presentationOwner = { kind: "policy", policy };
    if (retainedSelection !== undefined) {
      policy.select_manual(Number(retainedSelection.kind === "auto" ? retainedSelection.manual_scale : retainedSelection.scale), performance.now());
      policy.select_water_quality(retainedSelection.kind === "auto" ? retainedSelection.manual_water_quality : retainedSelection.water_quality, performance.now());
      if (retainedSelection.kind === "auto") policy.select_auto(true, performance.now());
    }
    let cachedScreenSnapshot: { readonly json: string; readonly value: ScreenRenderAutoSnapshot } | null = null;
    const screenSnapshot = (): ScreenRenderAutoSnapshot => {
      const json = policy.snapshot_json();
      if (cachedScreenSnapshot === null || cachedScreenSnapshot.json !== json) {
        cachedScreenSnapshot = { json, value: JSON.parse(json) as ScreenRenderAutoSnapshot };
      }
      return cachedScreenSnapshot.value;
    };
    dispatch({ type: "screen-render-observed", snapshot: screenSnapshot() });
    const bundle = createThreeRenderer(canvas, panelCanvas, navigator.xr ?? null,
      screen_vertical_field_of_view_degrees(), model.lakeWaterQuality.applied, headHudCanvas, {
        policy, snapshot: screenSnapshot, scene: () => gameSessionPhase(model.gameSession) === "boot" ? "Boot" : model.sharedGameUi?.scene ?? "Boot", onSnapshot: (snapshot) => {
          if (flightRenderer === bundle.renderer) dispatch({ type: "screen-render-observed", snapshot });
        }
      });
    presentationOwner = { kind: "renderer", renderer: bundle.renderer };
    const initializedSession = await initializeAppSession();
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
        observePresentationFrame,
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
    window.addEventListener("blur", cancelUnavailableCountdown);
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
      status: ""
    });
    syncGameSession();
  } catch (error) {
    window.removeEventListener("resize", onResize);
    window.removeEventListener("blur", cancelUnavailableCountdown);
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
          case "policy": ownedPresentation.policy.free(); return;
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
  if (gameSessionPhase(model.gameSession) === "boot") return createBootViewModel(model);
  if (model.sharedGameUi === null) return createBootViewModel(model);
  return createGameViewModel(model.sharedGameUi);
}

function sharedGameUiPlatformInput(): GameUiPlatformObservation {
  if (model.screenRenderScale === null) throw new Error("Shared Screen rendering observation is unavailable");
  const viewport = currentViewport();
  const presentationMode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
  const quality = model.lakeWaterQuality;
  const waterQualityOperation: GameUiPlatformInput["water_quality_operation"] = quality.kind === "applying"
    ? { kind: "applying", requested: quality.requested }
    : quality.kind === "failed" ? { kind: "failed", requested: quality.requested, cause: quality.message }
      : quality.cleanup.kind === "failed" ? { kind: "cleanup_failed", cause: quality.cleanup.message }
        : { kind: "idle" };
  const notification = [model.status, gameSessionPhase(model.gameSession) === "title" ? recordListNotice(model.recordList) : "",
    model.flightLogDownload.kind === "failed" || model.flightLogDownload.kind === "requested"
    ? model.flightLogDownload.message : "", model.lakeWaterQuality.kind === "failed" ? model.lakeWaterQuality.message : ""]
    .filter((value) => value.length > 0).join(" · ");
  return Object.freeze({
    viewport: Object.freeze({ width: viewport.x, height: viewport.y,
      root_font_size: Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize) }),
    chart_extent: presentationMode === "screen" ? screenUi.chartExtent() : null,
    presentation_mode: presentationMode === "phone-vr" ? "phone_vr" : presentationMode,
    operation_pending: model.pendingGameRequestId !== null || model.presentation.type !== "ready",
    webxr_available: model.webXrAvailable, phone_vr_available: model.phoneVrAvailable,
    analysis_available: true, replay_available: true,
    supported_replay_cameras: Object.freeze(["auto", "pilot", "chase", "orbit", "platform", "shore", "overhead", "side", "front", "telephoto"] as const),
    stored_records_available: true, flight_log_available: true,
    flight_log_pending: model.flightLogDownload.kind === "pending",
    water_quality: quality.applied,
    water_quality_operation: waterQualityOperation,
    screen_render_scale: model.screenRenderScale,
    notification,
    flight_controller_failure: model.flightExecution.kind === "stopped"
      && ["flight_running", "flight_paused"].includes(gameSessionPhase(model.gameSession)) ? model.flightExecution.message : null,
    head_hud_failure: headHudResourceFailure,
    stored_records: Object.freeze(recordListEntries(model.recordList).map((record) => Object.freeze({ id: record.id,
      saved_at: record.savedAt, personal_best: record.personalBest })))
  });
}

function synchronizeSharedGameUi(): void {
  const session = gameSession;
  if (session === null || gameSessionPhase(model.gameSession) === "boot") return;
  const view = session.readSharedUi(sharedGameUiPlatformInput());
  const transition = updateApp(model, { type: "shared-game-ui-observed", view });
  model = transition.model;
  for (const effect of transition.effects) runEffect(effect);
}

function runSharedGameSessionCommand(command: SessionCommand, requestId: number): void {
  const requestedPhase = gameSessionPhase(model.gameSession);
  const session = gameSession;
  if (session === null) {
    dispatch({ type: "game-operation-failed", requestId, message: "Rust GameSession is unavailable" });
    return;
  }
  try {
    const result = session.executeSharedSessionCommand(command);
    if (result.kind === "countdown-started") {
      const observedAt = performance.now();
      completeGameOperation(requestId);
      scheduleCountdownObservation(session, observedAt);
      return;
    }
    if (command.kind === "pause") flightController?.suspend();
    if (command.kind === "resume") flightController?.resume();
    if (result.kind === "aborted") {
      runSideEffects([
        () => { flightController?.synchronizeSnapshot(session.readSnapshot()); },
        () => { completeGameOperation(requestId); }
      ]);
      return;
    }
    completeGameOperation(requestId);
  } catch (error: unknown) {
    dispatchGameOperationFailure(requestId, errorMessage(error), { command, requestedPhase });
  }
}

function dispatchGameOperationFailure(
  requestId: number,
  message: string,
  context?: { readonly command: SessionCommand; readonly requestedPhase: GamePhase }
): void {
  const session = gameSession;
  if (session === null) {
    dispatch({ type: "game-operation-failed", requestId, message });
    return;
  }
  const dataset = flightAnalysisData(model.analysis);
  try {
    const currentSession = readRuntimeSessionProjection(session);
    const phase = currentSession.phase;
    if (phase === "result" && model.gameSession.phase === "result" && model.pendingGameRequestId !== requestId) {
      dispatch({ type: "game-session-status", message: `終端表示処理に失敗した: ${message}` });
      return;
    }
    const failureMessage = context === undefined
      ? message
      : `${context.command.kind} rejected: UI phase ${context.requestedPhase}, Rust phase ${phase}; ${message}`;
    dispatch({ type: "game-operation-failed", requestId, message: failureMessage, currentSession,
      analysisSourceInvalidated: dataset !== null && !session.isAnalysisDatasetCurrent(dataset) });
  } catch (syncError: unknown) {
    dispatch({
      type: "game-operation-failed",
      requestId,
      analysisSourceInvalidated: dataset !== null && !session.isAnalysisDatasetCurrent(dataset),
      message: `${message}; GameSession resync failed: ${errorMessage(syncError)}`
    });
  }
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

function scheduleCountdownObservation(owner: TailAppSessionFacade, previousMilliseconds: number): void {
  const token = owner.captureCountdownToken();
  if (token === null) return;
  let consumed = false;
  window.setTimeout(() => {
    if (consumed) return;
    consumed = true;
    if (gameSession !== owner) return;
    try {
      const currentToken = owner.captureCountdownToken();
      if (currentToken === null || currentToken.attempt !== token.attempt) return;
      const observedAt = performance.now();
      const frameAvailable = document.visibilityState !== "hidden" && document.hasFocus()
        && model.presentation.type === "ready" && runtime !== null && runtime.currentMode !== null
        && lastPresentationFrameMilliseconds !== null
        && observedAt - lastPresentationFrameMilliseconds <= maximum_flight_frame_gap_milliseconds();
      const progress = owner.observeCountdown(currentToken, (observedAt - previousMilliseconds) / 1000, frameAvailable);
      if (progress.kind === "ignored") return;
      syncGameSession();
      if (progress.kind === "launched") {
        if (flightController === null) createFlightController(owner);
        else resetFlightController();
      } else if (progress.kind === "waiting") {
        scheduleCountdownObservation(owner, observedAt);
      }
    } catch (error: unknown) {
      syncGameSession();
      dispatch({ type: "game-session-status", message: `発進準備に失敗した: ${errorMessage(error)}` });
    }
  }, 1000);
}

function observePresentationFrame(): void {
  const observedAt = performance.now();
  const available = document.visibilityState !== "hidden" && document.hasFocus()
    && (lastPresentationFrameMilliseconds === null
      || observedAt - lastPresentationFrameMilliseconds <= maximum_flight_frame_gap_milliseconds());
  lastPresentationFrameMilliseconds = observedAt;
  if (!available) cancelUnavailableCountdown();
  flightController?.onFrame(observedAt);
}

function cancelUnavailableCountdown(): void {
  const session = gameSession;
  if (session === null) return;
  const token = session.captureCountdownToken();
  if (token === null) return;
  try {
    session.observeCountdown(token, 0, false);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `発進準備の取消に失敗した: ${errorMessage(error)}` });
  }
}

function createFlightController(session: TailAppSessionFacade): void {
  const renderer = flightRenderer;
  if (renderer === null) throw new Error("Flight renderer is unavailable");
  const binding = flightControllerUi.bindAppDisplay(session.flightPort, controllerHudDisplay,
    () => session.readGameSessionProjection(), () => controller.currentSnapshot, dispatch);
  const input = new BrowserTailPilotInput(window, { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION,
    keyboard: { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION.keyboard, physicsHz } });
  let controller: TailFlightController;
  try {
    controller = new TailFlightController(session.flightPort, input, renderer, binding.port, physicsHz,
      maximum_flight_frame_gap_milliseconds(), undefined, binding.onTerminal, syncGameSession);
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
    () => session.readGameSessionProjection(), () => controller.currentSnapshot, dispatch);
  try { controller.reset(session.readSnapshot(), binding.port, binding.onTerminal); }
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
  if (projection.phase === "flight_running" || projection.phase === "flight_paused") {
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
    if (gameSession === session && session.acceptQuery(token, saved).kind === "accepted" && session.readLifecycle().phase === "result") {
      const personalBestMessage = saved.personalBest.kind === "candidate"
        ? "自己ベストを更新した"
        : saved.personalBest.kind === "existing"
          ? `自己ベスト: 記録 ${String(saved.personalBest.id)}`
          : saved.personalBest.kind === "ineligible"
            ? "参考記録"
            : "";
      dispatch({
        type: "game-session-status",
        message: `飛行記録 ${String(saved.id)}を保存した${personalBestMessage.length > 0 ? `。${personalBestMessage}` : ""}`
      });
    }
  } catch (error: unknown) {
    if (gameSession === session && session.acceptQuery(token, undefined).kind === "accepted" && session.readLifecycle().phase === "result") {
      dispatch({ type: "game-session-status", message: `飛行記録を保存できなかった: ${errorMessage(error)}` });
    }
  }
}

function createFlightRecordRepository(): FlightRecordRepository {
  return new FlightRecordRepository(
    new IndexedDbFlightRecordPersistence(window.indexedDB, personalBestComparisonVersion()),
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
  const displayedPhase = gameSessionPhase(model.gameSession);
  if (session !== null && isGameFlowActivation(action)) {
    syncGameSession();
    const synchronizedPhase = gameSessionPhase(model.gameSession);
    if (isStaleGameFlowActivation(action, displayedPhase, synchronizedPhase)
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
  renderModel();
}

function onPageHide(): void {
  flightLogDownload.dispose();
  window.removeEventListener("resize", onResize);
  window.removeEventListener("blur", cancelUnavailableCountdown);
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
      phase: () => session.readLifecycle().phase,
      cancel_countdown: () => { session.executeSharedSessionCommand({ kind: "cancel_countdown" }); },
      pause: (reason: number) => {
        if (reason !== 1) throw new RangeError("Page suspension requires the page-hidden reason");
        session.pauseForReason("page_hidden");
      }
    };
    suspendPageFlight(pagePort, flightController, syncGameSession);
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
  cancelUnavailableCountdown();
  pauseSessionForReason("presentation_transition");
}

function resolvePresentationTransitionPause(): void {
  clearSessionPauseReason(2);
}

function pauseSessionForReason(reason: "page_hidden" | "presentation_transition"): void {
  const session = gameSession;
  if (session === null || !["flight_running", "flight_paused"].includes(session.readLifecycle().phase)) return;
  try {
    session.pauseForReason(reason);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `飛行の一時停止に失敗した: ${errorMessage(error)}` });
  }
}

function clearSessionPauseReason(reason: number): void {
  const session = gameSession;
  if (session === null || session.readLifecycle().phase !== "flight_paused") return;
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

function currentRuntimeEnvironment(): RuntimeEnvironmentProjection {
  return gameSession === null ? { kind: "unavailable", reason: "no_selection" }
    : projectRuntimeEnvironment(gameSession.readEnvironment());
}

function currentViewport(): ViewportSize {
  const ratio = Number.isFinite(window.devicePixelRatio) ? Math.min(window.devicePixelRatio, 2) : 1;
  return Object.freeze({ x: Math.max(1, window.innerWidth), y: Math.max(1, window.innerHeight), pixelRatio: ratio });
}

function runtimeErrorMessage(result: Exclude<RuntimeResult, { readonly ok: true }>): string {
  if (result.error.type === "cleanup-failed") return result.error.failures.map((failure) =>
    `${failure.operation}${failure.type === "backend-failed" ? ` (${failure.mode})` : ""}: ${failure.message}`).join("; ");
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
    case "screen": return "";
    case "webxr": return "VR表示に切り替えた。視線を留めるか、選択操作で決定する。";
    case "phone-vr": return "スマートフォンVRに切り替えた。メニューから正面の向きを再設定できる。";
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
