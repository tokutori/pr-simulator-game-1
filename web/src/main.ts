import "./styles.css";
import { createBootViewModel } from "./app/boot-view.js";
import { createGameViewModel } from "./app/game-view.js";
import { createInitialAppModel, gameSessionPhaseCode, gameSessionSnapshot, updateApp } from "./app/app-state.js";
import type { AppEffect, AppMessage, AppModel, GameSessionOperation } from "./app/app-state.js";
import { ScreenPresentationBackend } from "./presentation/screen-backend.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { browserPanelContext } from "./presentation/browser-canvas.js";
import { drawVrPanel, VR_PANEL_PIXELS } from "./presentation/vr-panel-canvas.js";
import { PresentationRuntime } from "./presentation/runtime.js";
import { WebXrPresentationBackend } from "./presentation/webxr-backend.js";
import { PhoneVrPresentationBackend } from "./presentation/phone-vr-backend.js";
import { createBrowserPhoneVrSensorPort } from "./presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "./presentation/phone-vr-gamepad-browser.js";
import { BrowserPilotInput, DEFAULT_PILOT_INPUT_CONFIGURATION } from "./game/browser-input.js";
import { FlightController } from "./game/flight-controller.js";
import { syntheticLakeVisualCondition } from "./game/synthetic-lake-condition.js";
import { initializeGameSession } from "./game/wasm-flight.js";
import { FlightRecordRepository, IndexedDbFlightRecordPersistence } from "./game/flight-record-store.js";
import {
  loadFlightAnalysis,
  queryFlightRecordRenderPoseAt,
  queryFlightRecordSampleAt
} from "./game/flight-record-query.js";
import { parseFlightSnapshot } from "./game/flight-snapshot.js";
import { syntheticVenueMapForScenario } from "./game/synthetic-venue-map.js";
import { FlightHudAdapter } from "./presentation/flight-hud.js";
import { resolveAttractCameraMode, resolveReplayCameraMode } from "./render/camera/camera-director.js";
import { cinematicCameraView, isCinematicCameraMode } from "./render/camera/cinematic-camera.js";
import type { FlightSnapshot } from "./game/flight-snapshot.js";
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
const flightHudRoot = document.createElement("section");
stage.append(canvas, uiRoot, flightHudRoot);
mount.replaceChildren(stage);

const panelCanvas = document.createElement("canvas");
panelCanvas.width = VR_PANEL_PIXELS.width;
panelCanvas.height = VR_PANEL_PIXELS.height;
let model: AppModel = createInitialAppModel();
let runtime: PresentationRuntime | null = null;
let flightController: FlightController | null = null;
let gameSession: Awaited<ReturnType<typeof initializeGameSession>>["session"] | null = null;
let physicsHz = 0;
let flightRenderer: RendererAdapter | null = null;
let countdownGeneration = 0;
let webXrBackend: WebXrPresentationBackend | null = null;
let phoneVrBackend: PhoneVrPresentationBackend | null = null;
const flightHud = new FlightHudAdapter(flightHudRoot, (snapshot) => {
  if (model.presentation.type !== "ready" || model.presentation.mode === "screen") return;
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  if ((phaseCode !== 5 && phaseCode !== 6) || snapshot.tick % 3 !== 0) return;
  const panel = createGameViewModel(model, snapshot, model.flightAnalysis).panels[0];
  const context = panelCanvas.getContext("2d");
  if (panel !== undefined && context !== null) {
    drawVrPanel(browserPanelContext(context), panel, VR_PANEL_PIXELS.width, VR_PANEL_PIXELS.height);
  }
});
const screenUi = new ScreenUiAdapter(uiRoot, (action) => {
  dispatch({ type: "ui-action", action });
});

function dispatch(message: AppMessage): void {
  const previousPhaseCode = gameSessionPhaseCode(model.gameSession);
  const transition = updateApp(model, message);
  model = transition.model;
  if ([9, 10].includes(previousPhaseCode) && ![9, 10].includes(gameSessionPhaseCode(model.gameSession))) flightController?.renderCurrentSnapshot();
  renderModel();
  for (const effect of transition.effects) runEffect(effect);
}

function renderModel(): void {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  // The attract record is a separate, fixed scenario-1 flight. Its water
  // condition must not inherit the player's last setup selection.
  const weatherCode = phaseCode === 10 ? 0 : model.configurationMetadata?.weatherCode ?? model.difficulty.weatherCode;
  flightRenderer?.setLakeVisualCondition(syntheticLakeVisualCondition(weatherCode));
  flightHud.setInformationCode(model.difficulty.informationCode);
  flightHud.setVisible(phaseCode === 5 || phaseCode === 6);
  const presentationMode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
  const cameraMode = phaseCode === 10
    ? resolveAttractCameraMode(model.flightAnalysis, model.analysisCursorTimeSeconds, presentationMode)
    : phaseCode === 9
      ? resolveReplayCameraMode(model.replayCameraMode, model.flightAnalysis, model.analysisCursorTimeSeconds, presentationMode)
      : "pilot";
  const cameraPoints = syntheticVenueMapForScenario(model.configurationMetadata?.scenarioId ?? 1)?.cameraPoints ?? [];
  const cinematicView = isCinematicCameraMode(cameraMode) && model.replayPose !== null
    ? cinematicCameraView(cameraMode, model.replayPose, model.analysisCursorTimeSeconds, cameraPoints, model.flightAnalysis?.samples)
    : null;
  flightRenderer?.setCinematicCameraView(cinematicView);
  flightRenderer?.setFlightCameraMode(cameraMode);
  if (phaseCode === 9 || phaseCode === 10) {
    flightRenderer?.setFlightPose(model.replayPose);
  } else if (phaseCode <= 3 || phaseCode === 8 || (phaseCode === 7 && flightController === null)) {
    flightRenderer?.setFlightPose(null);
  }
  if (model.presentation.type === "hidden") {
    screenUi.clear();
    return;
  }
  const viewModel = currentViewModel();
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
      pauseForPresentationTransition();
      flightController?.suspend();
      void presentation.switchTo(effect.mode).then((result) => {
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, result);
        resolvePresentationTransitionPause();
      }, (error: unknown) => {
        dispatchBackendResult(effect.requestId, effect.mode, presentation.currentMode, {
          ok: false,
          error: { type: "renderer-failed", message: errorMessage(error) }
        });
        resolvePresentationTransitionPause();
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
      flightRenderer = null;
      void presentation?.dispose();
      return;
    }
    case "game-session-operation":
      runGameSessionOperation(effect.operation, effect.requestId);
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
      void createFlightRecordRepository().load(effect.id).then((json) => {
        if (json === null) throw new Error(`FlightRecord ${String(effect.id)} does not exist`);
        if (gameSession !== session || model.pendingGameRequestId !== effect.requestId) return;
        session.open_archived_flight_record(json);
        completeGameOperation(effect.requestId);
      }).catch((error: unknown) => {
        dispatch({ type: "game-operation-failed", requestId: effect.requestId, message: errorMessage(error) });
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
        // Archived records retain sampled wind but no live scenario wind field.
        const scenarioId = session.is_archived_replay()
          ? null
          : readConfigurationMetadata(session)?.scenarioId ?? null;
        const analysis = loadFlightAnalysis(session, physicsHz, scenarioId);
        dispatch({ type: "flight-analysis-loaded", requestId: effect.requestId, data: analysis });
      } catch (error: unknown) {
        dispatch({ type: "flight-analysis-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "load-flight-analysis-cursor": {
      const session = gameSession;
      if (session === null) {
        dispatch({ type: "flight-analysis-cursor-failed", requestId: effect.requestId, message: "Rust GameSession is unavailable" });
        return;
      }
      try {
        const sample = queryFlightRecordSampleAt(session, physicsHz, effect.timeSeconds);
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
        const pose = queryFlightRecordRenderPoseAt(
          session,
          physicsHz,
          effect.timeSeconds,
          analysis.initialPilotPositionMeters
        );
        dispatch({ type: "flight-replay-pose-loaded", requestId: effect.requestId, pose });
      } catch (error: unknown) {
        dispatch({ type: "flight-replay-pose-failed", requestId: effect.requestId, message: errorMessage(error) });
      }
      return;
    }
    case "schedule-replay-clock-tick": {
      const startedAt = performance.now();
      window.setTimeout(() => {
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

async function initializePresentation(requestId: number): Promise<void> {
  let rendererAdapter: RendererAdapter | null = null;
  try {
    const { createThreeRenderer } = await import("./render/engines/three/three-renderer.js");
    if (model.presentation.type === "hidden") return;
    const bundle = createThreeRenderer(canvas, panelCanvas, navigator.xr ?? null);
    rendererAdapter = bundle.renderer;
    const initializedSession = await initializeGameSession();
    if (isPageHidden()) {
      initializedSession.session.free();
      bundle.renderer.dispose();
      return;
    }
    gameSession = initializedSession.session;
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
      currentViewModel,
      (timestampMs) => flightController?.onFrame(timestampMs)
    );
    runtime = presentation;
    const started = await presentation.start("screen");
    if (!started.ok) {
      const status = `Renderer initialization failed: ${runtimeErrorMessage(started)}`;
      await presentation.dispose();
      gameSession.free();
      gameSession = null;
      rendererAdapter = null;
      flightRenderer = null;
      if (runtime === presentation) runtime = null;
      dispatch({ type: "presentation-initialization-failed", requestId, message: status });
      return;
    }
    if (isPageHidden()) {
      await presentation.dispose();
      gameSession.free();
      gameSession = null;
      rendererAdapter = null;
      flightRenderer = null;
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
    dispatch({
      type: "game-session-synced",
      phaseCode: gameSession.phase_code(),
      controlModeCode: gameSession.control_mode_code(),
      difficulty: readDifficulty(gameSession),
      configurationMetadata: readConfigurationMetadata(gameSession),
      countdownRemaining: gameSession.countdown_remaining(),
      canResume: gameSession.can_resume(),
      snapshot: null
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
    flightController?.dispose();
    flightController = null;
    gameSession?.free();
    gameSession = null;
    flightRenderer = null;
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

function currentViewModel() {
  if (gameSessionPhaseCode(model.gameSession) < 0) return createBootViewModel(model);
  return createGameViewModel(
    model,
    flightController?.currentSnapshot ?? gameSessionSnapshot(model.gameSession),
    model.flightAnalysis
  );
}

function runGameSessionOperation(operation: GameSessionOperation, requestId: number): void {
  const session = gameSession;
  if (session === null) {
    dispatch({ type: "game-operation-failed", requestId, message: "Rust GameSession is unavailable" });
    return;
  }
  try {
    switch (operation) {
      case "open-setup":
        session.open_setup();
        break;
      case "set-control-manual":
        session.set_control_mode(0);
        break;
      case "set-control-shared":
        session.set_control_mode(1);
        break;
      case "set-control-automatic":
        session.set_control_mode(2);
        break;
      case "cycle-difficulty-preset":
        session.cycle_difficulty_preset();
        break;
      case "cycle-information-level":
        session.cycle_information_level();
        break;
      case "cycle-assistance-level":
        session.cycle_assistance_level();
        break;
      case "cycle-weather-class":
        session.cycle_weather_class();
        break;
      case "return-to-title":
        session.return_to_title();
        break;
      case "prepare":
        session.prepare();
        session.mark_briefing_ready();
        break;
      case "cancel-briefing":
        session.cancel_briefing();
        break;
      case "start-flight":
        session.start_countdown(3);
        completeGameOperation(requestId);
        countdownGeneration += 1;
        scheduleCountdownTick(countdownGeneration);
        return;
      case "cancel-countdown":
        countdownGeneration += 1;
        session.cancel_countdown();
        break;
      case "pause":
        session.pause(0);
        flightController?.suspend();
        break;
      case "resume":
        session.resume();
        flightController?.resume();
        break;
      case "abort": {
        const terminal = session.abort();
        flightController?.reset(terminal);
        completeGameOperation(requestId, terminal);
        return;
      }
      case "retry":
        session.retry();
        break;
      case "retry-briefing":
        session.retry_briefing();
        session.mark_briefing_ready();
        break;
      case "enter-replay":
        (session as unknown as { enter_replay(): void }).enter_replay();
        break;
      case "leave-replay":
        (session as unknown as { leave_replay(): void }).leave_replay();
        break;
      case "enter-attract":
        session.enter_attract();
        break;
      case "leave-attract":
        session.leave_attract();
        break;
      default:
        return assertNever(operation);
    }
    completeGameOperation(requestId);
  } catch (error: unknown) {
    dispatch({ type: "game-operation-failed", requestId, message: errorMessage(error) });
  }
}

function completeGameOperation(requestId: number, rawSnapshot?: ArrayLike<number>): void {
  const session = gameSession;
  if (session === null) return;
  const snapshot = rawSnapshot === undefined
    ? flightController?.currentSnapshot ?? gameSessionSnapshot(model.gameSession)
    : parseFlightSnapshot(rawSnapshot);
  dispatch({
    type: "game-operation-completed",
    requestId,
    phaseCode: session.phase_code(),
    controlModeCode: session.control_mode_code(),
    difficulty: readDifficulty(session),
    configurationMetadata: readConfigurationMetadata(session),
    countdownRemaining: session.countdown_remaining(),
    canResume: session.can_resume(),
    snapshot
  });
}

function scheduleCountdownTick(generation: number): void {
  window.setTimeout(() => {
    const session = gameSession;
    if (session === null || generation !== countdownGeneration || session.phase_code() !== 4) return;
    try {
      const remainingTicks = session.advance_countdown();
      dispatch({
        type: "game-session-synced",
        phaseCode: session.phase_code(),
        controlModeCode: session.control_mode_code(),
        difficulty: readDifficulty(session),
        configurationMetadata: readConfigurationMetadata(session),
        countdownRemaining: remainingTicks,
        canResume: session.can_resume(),
        snapshot: gameSessionSnapshot(model.gameSession)
      });
      if (remainingTicks === 0) {
        const initial = session.launch();
        if (flightController === null) createFlightController(session);
        else flightController.reset(initial);
        dispatch({
          type: "game-session-synced",
          phaseCode: session.phase_code(),
          controlModeCode: session.control_mode_code(),
          difficulty: readDifficulty(session),
          configurationMetadata: readConfigurationMetadata(session),
          countdownRemaining: 0,
          canResume: session.can_resume(),
          snapshot: parseFlightSnapshot(initial)
        });
      } else {
        scheduleCountdownTick(generation);
      }
    } catch (error: unknown) {
      dispatch({
        type: "game-session-synced",
        phaseCode: session.phase_code(),
        controlModeCode: session.control_mode_code(),
        difficulty: readDifficulty(session),
        configurationMetadata: readConfigurationMetadata(session),
        countdownRemaining: session.countdown_remaining(),
        canResume: session.can_resume(),
        snapshot: gameSessionSnapshot(model.gameSession)
      });
      dispatch({ type: "game-session-status", message: `発進準備に失敗した: ${errorMessage(error)}` });
    }
  }, 800);
}

function createFlightController(
  session: Awaited<ReturnType<typeof initializeGameSession>>["session"]
): void {
  const renderer = flightRenderer;
  if (renderer === null) throw new Error("Flight renderer is unavailable");
  flightController = new FlightController(
    session,
    new BrowserPilotInput(window, {
      ...DEFAULT_PILOT_INPUT_CONFIGURATION,
      physicsHz
    }),
    renderer,
    flightHud,
    physicsHz,
    undefined,
    (snapshot: FlightSnapshot) => {
      const activeSession = gameSession;
      if (activeSession === null) return;
      dispatch({
        type: "game-session-synced",
        phaseCode: activeSession.phase_code(),
        controlModeCode: activeSession.control_mode_code(),
        difficulty: readDifficulty(activeSession),
        configurationMetadata: readConfigurationMetadata(activeSession),
        countdownRemaining: activeSession.countdown_remaining(),
        canResume: activeSession.can_resume(),
        snapshot
      });
    }
  );
}

async function persistFlightRecord(
  session: NonNullable<typeof gameSession>
): Promise<void> {
  try {
    const repository = createFlightRecordRepository();
    const saved = await repository.saveFrom(session);
    dispatch({ type: "refresh-stored-flight-records" });
    if (gameSession === session && session.phase_code() === 7) {
      dispatch({ type: "game-session-status", message: `FlightRecord ${String(saved.id)}を保存した` });
    }
  } catch (error: unknown) {
    if (gameSession === session && session.phase_code() === 7) {
      dispatch({ type: "game-session-status", message: `FlightRecord保存に失敗した: ${errorMessage(error)}` });
    }
  }
}

function createFlightRecordRepository(): FlightRecordRepository {
  return new FlightRecordRepository(new IndexedDbFlightRecordPersistence(window.indexedDB));
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
  document.removeEventListener("visibilitychange", onVisibilityChange);
  if (flightController !== null) flightController.dispose();
  else gameSession?.free();
  flightController = null;
  gameSession = null;
  dispatch({ type: "page-hidden" });
}

function onVisibilityChange(): void {
  const session = gameSession;
  if (document.visibilityState === "hidden") {
    if (session?.phase_code() === 4) {
      countdownGeneration += 1;
      session.cancel_countdown();
      syncGameSession();
      return;
    }
    pauseSessionForReason(1);
    flightController?.suspend();
    return;
  }
  clearSessionPauseReason(1);
}

function pauseForPresentationTransition(): void {
  pauseSessionForReason(2);
}

function resolvePresentationTransitionPause(): void {
  clearSessionPauseReason(2);
}

function pauseSessionForReason(reason: number): void {
  const session = gameSession;
  if (session === null || (session.phase_code() !== 5 && session.phase_code() !== 6)) return;
  try {
    session.pause(reason);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `飛行の一時停止に失敗した: ${errorMessage(error)}` });
  }
}

function clearSessionPauseReason(reason: number): void {
  const session = gameSession;
  if (session === null || session.phase_code() !== 6) return;
  try {
    session.clear_pause_reason(reason);
    syncGameSession();
  } catch (error: unknown) {
    dispatch({ type: "game-session-status", message: `一時停止状態の更新に失敗した: ${errorMessage(error)}` });
  }
}

function syncGameSession(): void {
  const session = gameSession;
  if (session === null) return;
  dispatch({
    type: "game-session-synced",
    phaseCode: session.phase_code(),
    controlModeCode: session.control_mode_code(),
    difficulty: readDifficulty(session),
    configurationMetadata: readConfigurationMetadata(session),
    countdownRemaining: session.countdown_remaining(),
    canResume: session.can_resume(),
    snapshot: flightController?.currentSnapshot ?? gameSessionSnapshot(model.gameSession)
  });
}

function readDifficulty(session: NonNullable<typeof gameSession>): AppModel["difficulty"] {
  return Object.freeze({
    presetCode: session.difficulty_preset_code(),
    informationCode: session.information_level_code(),
    assistanceCode: session.assistance_level_code(),
    weatherCode: session.weather_class_code()
  });
}

function readConfigurationMetadata(
  session: NonNullable<typeof gameSession>
): AppModel["configurationMetadata"] {
  const phaseCode = session.phase_code();
  if (phaseCode < 2 || phaseCode > 9) return null;
  // A newly opened archive can replace a Result's configuration. Once Replay
  // has been synchronized, its immutable identity may be reused.
  if (model.configurationMetadata !== null &&
      (!session.is_archived_replay() || gameSessionPhaseCode(model.gameSession) === 9)) {
    return model.configurationMetadata;
  }
  const values = session.configuration_metadata();
  if (values.length !== 12) throw new RangeError("Resolved configuration metadata has an invalid length");
  const valueAt = (index: number): number => {
    const value = values[index];
    if (value === undefined || !Number.isInteger(value) || value < 0) {
      throw new RangeError("Resolved configuration metadata contains an invalid value");
    }
    return value;
  };
  return Object.freeze({
    presetCode: valueAt(0),
    informationCode: valueAt(1),
    assistanceCode: valueAt(2),
    weatherCode: valueAt(3),
    catalogVersion: valueAt(4),
    scenarioId: valueAt(5),
    scenarioVersion: valueAt(6),
    aircraftModelVersion: valueAt(7),
    environmentVersion: valueAt(8),
    controllerProfileVersion: valueAt(9),
    seedLow: valueAt(10),
    seedHigh: valueAt(11)
  });
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
document.addEventListener("visibilitychange", onVisibilityChange);
dispatch({ type: "initialize" });
