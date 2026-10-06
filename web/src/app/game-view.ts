import { IDENTITY_POSE, pose, vec3 } from "../render/contracts/math.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import { FLIGHT_MENU_GEOMETRY, normalizedRect } from "../render/contracts/ui.js";
import type { UiButton, UiChart, UiPanel, UiRange, UiStatus, UiToggle, UiViewModel } from "../render/contracts/ui.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightAnalysisData } from "../game/flight-record-query.js";
import { venueMapForScenario } from "../game/biwa-venue-map.js";
import { NO_HEAD_HUD_VIEW } from "../presentation/head-hud-view.js";
import type { HeadHudUnavailableReason, HeadHudView } from "../presentation/head-hud-view.js";
import type {
  AppModel,
  ConfigurationMetadataUiState,
  DifficultyUiState,
  PresentationUiState
} from "./app-state.js";
import { gameSessionCountdown, gameSessionPhaseCode } from "./app-state.js";

export function createGameViewModel(
  model: AppModel,
  snapshot: FlightSnapshot | null,
  analysis: FlightAnalysisData | null = model.flightAnalysis,
  headHudView: HeadHudView = NO_HEAD_HUD_VIEW
): UiViewModel {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  const countdownRemaining = gameSessionCountdown(model.gameSession);
  const canResume = model.gameSession.kind === "paused-flight" && model.gameSession.canResume;
  const pauseOverlay = model.gameSession.kind === "paused-flight" ? model.gameSession.overlay.kind : null;
  const scene = sceneForPhase(phaseCode);
  const buttons = gameButtons(
    phaseCode,
    model.difficulty,
    model.configurationMetadata,
    countdownRemaining,
    model.flightAnalysis !== null,
    canResume,
    pauseOverlay
  );
  if (phaseCode === 0) {
    const savedFlightButtons = model.storedFlightRecords.slice(0, 3).map((record) => button(
      `game-title-open-record-${String(record.id)}`,
      `${record.personalBest ? "Personal Best · " : ""}Replay #${String(record.id)} · ${formatSavedAt(record.savedAt)}`,
      model.pendingGameRequestId === null
    ));
    buttons.splice(1, 0, ...savedFlightButtons);
  }
  const activeMode = activeModeOf(model.presentation);
  if (phaseCode === 0) {
    buttons.push(
      button("boot-enter-webxr", model.webXrAvailable ? "WebXRへ切替" : "WebXRは利用できない", model.webXrAvailable && canRequest(model.presentation)),
      button("boot-enter-phone-vr", model.phoneVrAvailable ? "Phone VRへ切替" : "Phone VRは利用できない", model.phoneVrAvailable && canRequest(model.presentation))
    );
  }
  if (phaseCode === 0 && activeMode === "phone-vr") buttons.push(button("boot-recenter-phone-tracking", "頭部追跡を再設定", true));
  if (phaseCode === 0 && (activeMode === "webxr" || activeMode === "phone-vr")) {
    buttons.push(button("boot-recenter-menu", "メニューを正面に配置", true));
    buttons.push(button("boot-exit-vr", "VRを終了", true));
  }

  const vrFlightPanel = (phaseCode === 5 || phaseCode === 6) && (activeMode === "webxr" || activeMode === "phone-vr");
  const controls: (UiButton | UiToggle | UiRange | UiStatus | UiChart)[] = phaseCode === 5 || phaseCode === 9 || phaseCode === 10 || vrFlightPanel
    ? []
    : [status("game-state", "状態", model.status || descriptionForPhase(phaseCode, model.difficulty, countdownRemaining, canResume))];
  const flightButtons = buttons.filter((entry) => entry.id === "game-flight-pause" || entry.id === "game-flight-resume" || entry.id === "game-flight-abort" || entry.id === "game-paused-abort");
  if (phaseCode === 5 || phaseCode === 6) {
    if (!vrFlightPanel) {
      if (phaseCode === 6 && pauseOverlay === "settings") {
        controls.push(status("game-pause-settings-info", "Flight Settings", "飛行中は難易度・操縦bindingを固定する。変更する場合は飛行を終了してFlightSetupへ戻る。"));
      } else if (phaseCode === 6 && pauseOverlay === "help") {
        controls.push(status("game-pause-help-info", "操縦方法", "Roll A / D · Pitch ↑ / ↓ · Yaw ← / → · 重心 J / L"));
      }
      (phaseCode === 6 ? buttons : flightButtons).forEach((entry) => controls.push(entry));
    } else if (phaseCode === 5) {
      if (headHudView.kind === "unavailable") controls.push(Object.freeze({
        ...status("game-head-hud-unavailable", "情報板", headHudUnavailableLabel(headHudView.reason)),
        rect: normalizedRect(0.06, 0.2, 0.88, 0.42)
      }));
      const pause = flightButtons.find((entry) => entry.id === "game-flight-pause");
      if (pause !== undefined) controls.push(Object.freeze({
        ...pause, rect: headHudView.kind === "unavailable" ? normalizedRect(0.06, 0.7, 0.88, 0.18) : normalizedRect(0.06, 0.3, 0.88, 0.5)
      }));
    } else {
      if (pauseOverlay === "settings") {
        controls.push(Object.freeze({
          ...status("game-pause-settings-info", "Flight Settings", "飛行中は難易度・操縦bindingを固定する。変更する場合は飛行を終了してFlightSetupへ戻る。"),
          rect: normalizedRect(0.08, 0.24, 0.84, 0.25)
        }));
      } else if (pauseOverlay === "help") {
        controls.push(Object.freeze({
          ...status("game-pause-help-info", "操縦方法", "Roll A / D · Pitch ↑ / ↓ · Yaw ← / → · 重心 J / L"),
          rect: normalizedRect(0.08, 0.24, 0.84, 0.25)
        }));
      } else {
        controls.push(Object.freeze({ ...status("game-state", "状態", "一時停止中"), rect: normalizedRect(0.08, 0.2, 0.84, 0.12) }));
      }
      buttons.forEach((entry, index) => {
        controls.push(Object.freeze({
          ...entry,
          rect: normalizedRect(0.08, pauseOverlay === "menu" ? 0.4 + index * 0.11 : 0.72, 0.84, 0.085)
        }));
      });
    }
  }
  if (phaseCode === 7 && model.resultTab === "summary") {
    const result = status("game-result-analysis", "Summary", resultAnalysisSummary(analysis));
    controls.push(Object.freeze({ ...result, rect: normalizedRect(0.08, 0.15, 0.84, 0.085) }));
  }
  if (phaseCode === 7 && model.resultTab === "analysis") {
    controls.push(
      Object.freeze({ ...button("game-analysis-map", model.analysisChart === "map" ? "● Map" : "Map", true), rect: normalizedRect(0.02, 0.025, 0.30, 0.06) }),
      Object.freeze({ ...button("game-analysis-altitude", model.analysisChart === "altitude" ? "● Altitude" : "Altitude", true), rect: normalizedRect(0.35, 0.025, 0.30, 0.06) }),
      Object.freeze({ ...button("game-analysis-speed", model.analysisChart === "speed" ? "● Speed" : "Speed", true), rect: normalizedRect(0.68, 0.025, 0.30, 0.06) })
    );
    controls.push(Object.freeze({
      ...status("game-result-configuration", "Configuration", configurationSummary(model.configurationMetadata)),
      rect: normalizedRect(0.04, 0.735, 0.92, 0.065)
    }));
    if (analysis !== null) controls.push(Object.freeze({
      kind: "range",
      id: "game-analysis-cursor",
      label: "Flight time (s)",
      value: model.analysisCursorTimeSeconds,
      minimum: 0,
      maximum: Math.max(analysis.summary.durationSeconds, 0.01),
      step: 0.01,
      enabled: true,
      rect: normalizedRect(0.04, 0.61, 0.92, 0.045)
    }));
    if (model.analysisCursorSample !== null) {
      controls.push(Object.freeze({
        ...status("game-analysis-cursor-values", "At cursor", cursorReadout(model.analysisCursorSample)),
        rect: normalizedRect(0.04, 0.66, 0.92, 0.065)
      }));
    }
    if (analysis === null) {
      controls.push(Object.freeze({ ...status("game-analysis-loading", "Analysis", "FlightRecordを取得している"), rect: normalizedRect(0.08, 0.105, 0.84, 0.49) }));
    } else {
      controls.push(createAnalysisChart(
        analysis,
        model.analysisChart,
        model.analysisCursorSample,
        model.configurationMetadata?.scenarioId ?? null,
        normalizedRect(0.04, 0.105, 0.92, 0.49)
      ));
    }
    controls.push(
      Object.freeze({ ...button("game-result-open-summary", "Summary", true), rect: normalizedRect(0.02, 0.82, 0.30, 0.07) }),
      Object.freeze({ ...button("game-result-replay", "Replay", analysis !== null), rect: normalizedRect(0.35, 0.82, 0.30, 0.07) }),
      Object.freeze({ ...button("game-result-retry", "同条件で再試行", true), rect: normalizedRect(0.68, 0.82, 0.30, 0.07) }),
      Object.freeze({ ...button("game-result-setup", "設定", true), rect: normalizedRect(0.18, 0.91, 0.30, 0.07) }),
      Object.freeze({ ...button("game-result-title", "Title", true), rect: normalizedRect(0.52, 0.91, 0.30, 0.07) })
    );
  } else if (phaseCode === 9 && model.replayViewMode === "analysis") {
    controls.push(
      Object.freeze({ ...button("game-replay-view-mode", "表示: Analysis", true), rect: normalizedRect(0.02, 0.035, 0.22, 0.06) }),
      Object.freeze({ ...button("game-analysis-map", model.analysisChart === "map" ? "● Map" : "Map", true), rect: normalizedRect(0.26, 0.035, 0.22, 0.06) }),
      Object.freeze({ ...button("game-analysis-altitude", model.analysisChart === "altitude" ? "● Altitude" : "Altitude", true), rect: normalizedRect(0.50, 0.035, 0.22, 0.06) }),
      Object.freeze({ ...button("game-analysis-speed", model.analysisChart === "speed" ? "● Speed" : "Speed", true), rect: normalizedRect(0.74, 0.035, 0.24, 0.06) })
    );
    if (analysis !== null) controls.push(createAnalysisChart(
      analysis,
      model.analysisChart,
      model.analysisCursorSample,
      model.configurationMetadata?.scenarioId ?? null,
      normalizedRect(0.04, 0.12, 0.92, 0.59)
    ));
    else controls.push(Object.freeze({
      ...status("game-replay-analysis-loading", "Analysis", "FlightRecordを取得している"),
      rect: normalizedRect(0.08, 0.30, 0.84, 0.24)
    }));
    controls.push(
      Object.freeze({
        ...button("game-replay-play-pause", model.replayPlaying ? "Pause" : "Play", analysis !== null),
        rect: normalizedRect(0.04, 0.74, 0.15, 0.06)
      }),
      Object.freeze({
        kind: "range",
        id: "game-replay-cursor",
        label: "Replay time (s)",
        value: model.analysisCursorTimeSeconds,
        minimum: 0,
        maximum: Math.max(analysis?.summary.durationSeconds ?? 0, 0.01),
        step: 0.01,
        enabled: analysis !== null,
        rect: normalizedRect(0.21, 0.74, 0.75, 0.06)
      }),
      Object.freeze({
        ...status("game-replay-analysis-cursor-values", "At cursor", model.analysisCursorSample === null ? "記録時刻を読み込み中" : cursorReadout(model.analysisCursorSample)),
        rect: normalizedRect(0.04, 0.82, 0.92, 0.075)
      }),
      Object.freeze({
        ...button("game-replay-camera", `Camera: ${replayCameraLabel(model.replayCameraMode)}`, model.presentation.type === "ready"),
        rect: normalizedRect(0.04, 0.91, 0.32, 0.06)
      }),
      Object.freeze({ ...button("game-replay-return", "Resultへ戻る", true), rect: normalizedRect(0.64, 0.91, 0.32, 0.06) })
    );
  } else if (phaseCode === 9) {
    const durationSeconds = analysis?.summary.durationSeconds ?? 0;
    const isCinematic = model.replayViewMode === "cinematic";
    controls.push(Object.freeze({
      ...status("game-replay-time", "Record time", `${model.analysisCursorTimeSeconds.toFixed(2)} / ${durationSeconds.toFixed(2)} s`),
      rect: normalizedRect(0.04, 0.62, 0.92, 0.08)
    }));
    if (!isCinematic && analysis !== null) controls.push(Object.freeze({
      kind: "range",
      id: "game-replay-cursor",
      label: "Replay time (s)",
      value: model.analysisCursorTimeSeconds,
      minimum: 0,
      maximum: Math.max(durationSeconds, 0.01),
      step: 0.01,
      enabled: true,
      rect: normalizedRect(0.04, 0.52, 0.92, 0.07)
    }));
    if (!isCinematic && (model.pendingReplayPoseRequestId !== null || model.replayPose === null)) {
      controls.push(Object.freeze({
        ...status("game-replay-pose-status", "Replay", "記録済みposeを取得している"),
        rect: normalizedRect(0.04, 0.40, 0.92, 0.08)
      }));
    }
    controls.push(
      Object.freeze({
        ...button("game-replay-view-mode", `表示: ${isCinematic ? "Cinematic" : "Telemetry"}`, true),
        rect: normalizedRect(0.04, 0.84, 0.92, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-play-pause", model.replayPlaying ? "Pause" : "Play", analysis !== null),
        rect: normalizedRect(0.04, 0.31, 0.30, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-camera", `Camera: ${replayCameraLabel(model.replayCameraMode)}`, model.presentation.type === "ready"),
        rect: normalizedRect(0.36, 0.31, 0.60, 0.07)
      }),
      ...(!isCinematic ? [
        Object.freeze({
          ...button("game-replay-speed-0_5", model.replaySpeed === 0.5 ? "● 0.5×" : "0.5×", true),
          rect: normalizedRect(0.04, 0.22, 0.29, 0.07)
        }),
        Object.freeze({
          ...button("game-replay-speed-1", model.replaySpeed === 1 ? "● 1×" : "1×", true),
          rect: normalizedRect(0.355, 0.22, 0.29, 0.07)
        }),
        Object.freeze({
          ...button("game-replay-speed-2", model.replaySpeed === 2 ? "● 2×" : "2×", true),
          rect: normalizedRect(0.67, 0.22, 0.29, 0.07)
        })
      ] : [])
    );
    buttons.forEach((entry) => controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.04, 0.12, 0.92, 0.075) })));
  } else if (phaseCode === 10) {
    controls.push(
      Object.freeze({ ...status("game-attract-status", "Demo", model.status || "自動再生中"), rect: normalizedRect(0.04, 0.72, 0.92, 0.08) }),
      Object.freeze({
        ...status("game-attract-time", "Record time", `${model.analysisCursorTimeSeconds.toFixed(2)} / ${(analysis?.summary.durationSeconds ?? 0).toFixed(2)} s`),
        rect: normalizedRect(0.04, 0.62, 0.92, 0.07)
      }),
      ...(model.analysisCursorSample === null ? [] : [Object.freeze({
        ...status("game-attract-distance", "Distance", `${Math.hypot(model.analysisCursorSample.northMeters, model.analysisCursorSample.eastMeters).toFixed(1)} m`),
        rect: normalizedRect(0.04, 0.52, 0.92, 0.07)
      })]),
      Object.freeze({ ...button("game-attract-return", "Titleへ戻る", true), rect: normalizedRect(0.04, 0.40, 0.92, 0.075) })
    );
  } else if (phaseCode === 1 && model.difficulty.informationCode === 4) {
    const cueColumnById = new Map([
      ["game-setup-information-telemetry", 0],
      ["game-setup-information-attitude", 1],
      ["game-setup-information-wind", 0],
      ["game-setup-information-flight-path", 1],
      ["game-setup-information-angle-of-attack", 0],
      ["game-setup-information-warnings", 1]
    ]);
    let cueIndex = 0;
    buttons.forEach((entry) => {
      if (entry.id === "game-state") {
        controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.04, 0.025, 0.92, 0.055) }));
        return;
      }
      const cueColumn = cueColumnById.get(entry.id);
      if (cueColumn !== undefined) {
        const row = Math.floor(cueIndex / 2);
        cueIndex += 1;
        controls.push(Object.freeze({
          ...entry,
          rect: normalizedRect(0.04 + cueColumn * 0.48, 0.285 + row * 0.075, 0.44, 0.065)
        }));
        return;
      }
      const yById: Readonly<Record<string, number>> = {
        "game-setup-preset": 0.095,
        "game-setup-information": 0.175,
        "game-setup-assistance": 0.53,
        "game-setup-weather": 0.61,
        "game-setup-start": 0.71,
        "game-setup-back": 0.80
      };
      const y = yById[entry.id];
      if (y !== undefined) controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.08, y, 0.84, 0.07) }));
    });
  } else if (phaseCode !== 5 && phaseCode !== 6 && !(phaseCode === 7 && model.resultTab === "analysis")) {
    let bottom = 0.795;
    const totalHeight = buttons.reduce((total, entry) => total + (entry.id === "game-result-configuration" ? 0.11 : 0.075) + 0.01, 0);
    const scale = Math.min(1, 0.76 / totalHeight);
    buttons.forEach((entry) => {
      const height = (entry.id === "game-result-configuration" ? 0.11 : 0.075) * scale;
      const y = bottom - height;
      controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.08, y, 0.84, height) }));
      bottom = y - 0.01 * scale;
    });
  }
  const renderedControls = model.pendingGameRequestId === null
    ? controls
    : controls.map((control) => control.kind === "button" || control.kind === "range"
      ? Object.freeze({ ...control, enabled: false })
      : control);
  const panel: UiPanel = Object.freeze({
    id: "game-flow",
    title: vrFlightPanel ? phaseCode === 5 ? "Pause" : pauseOverlay === "settings" ? "Settings" : pauseOverlay === "help" ? "Help" : "Pause" : "ゲーム進行",
    anchor: "menu",
    localPose: vrFlightPanel && phaseCode === 5 ? pose(vec3(0, FLIGHT_MENU_GEOMETRY.centerY, 0), IDENTITY_POSE.orientation) : IDENTITY_POSE,
    size: vrFlightPanel && phaseCode === 5
      ? headHudView.kind === "unavailable" ? Object.freeze({ width: 0.9, height: 0.5 }) : Object.freeze({ width: FLIGHT_MENU_GEOMETRY.width, height: FLIGHT_MENU_GEOMETRY.height })
      : Object.freeze({ width: 2.4, height: 1.8 }),
    controls: Object.freeze(renderedControls)
  });
  return Object.freeze({
    scene,
    title: titleForScene(scene),
    description: phaseCode === 0
      ? [descriptionForPhase(phaseCode, model.difficulty, countdownRemaining, canResume),
        model.storedFlightRecordsStatus,
        model.storedFlightRecords.length > 0 ? `保存FlightRecord ${String(model.storedFlightRecords.length)}件` : ""]
        .filter(Boolean).join(" · ")
      : descriptionForPhase(phaseCode, model.difficulty, countdownRemaining, canResume),
    presentationStyle: phaseCode === 10 || (phaseCode === 9 && model.replayViewMode !== "analysis") ? "cinematic" : "default",
    activeOverlay: phaseCode === 6
      ? pauseOverlay === "settings" ? "PauseSettings" : pauseOverlay === "help" ? "PauseHelp" : "Pause"
      : null,
    panels: Object.freeze([panel]),
    headHud: vrFlightPanel && phaseCode === 5 && snapshot !== null && headHudView.kind === "visible" ? headHudView.layer : NO_HEAD_HUD
  });
}

function headHudUnavailableLabel(reason: HeadHudUnavailableReason): string {
  switch (reason) {
    case "not-stereo": return "両眼表示を確認する。";
    case "viewer-unavailable": return "頭部追跡の復帰を待つ。";
    case "unsupported-view-configuration": return "このVR表示形式では情報板を表示できない。";
    case "invalid-view-geometry": return "VRの投影情報を取得できない。";
    case "insufficient-view-area": return "両眼に収まる情報板の表示領域が不足している。";
    case "text-overflow": return "情報板の文字を読み取れる大きさで配置できない。";
  }
}

function clipMapSegment(
  start: Readonly<{ x: number; y: number }>,
  end: Readonly<{ x: number; y: number }>,
  minimumX: number,
  maximumX: number,
  minimumY: number,
  maximumY: number
): Readonly<{ start: Readonly<{ x: number; y: number }>; end: Readonly<{ x: number; y: number }> }> | null {
  const deltaX = end.x - start.x;
  const deltaY = end.y - start.y;
  let lower = 0;
  let upper = 1;
  for (const [p, q] of [
    [-deltaX, start.x - minimumX], [deltaX, maximumX - start.x],
    [-deltaY, start.y - minimumY], [deltaY, maximumY - start.y]
  ] as const) {
    if (p === 0) {
      if (q < 0) return null;
      continue;
    }
    const ratio = q / p;
    if (p < 0) lower = Math.max(lower, ratio);
    else upper = Math.min(upper, ratio);
    if (lower > upper) return null;
  }
  return {
    start: { x: start.x + lower * deltaX, y: start.y + lower * deltaY },
    end: { x: start.x + upper * deltaX, y: start.y + upper * deltaY }
  };
}

function clipMapPolyline(
  points: readonly Readonly<{ x: number; y: number }>[],
  minimumX: number,
  maximumX: number,
  minimumY: number,
  maximumY: number
): readonly (readonly Readonly<{ x: number; y: number }>[])[] {
  const fragments: Readonly<{ x: number; y: number }>[][] = [];
  let current: Readonly<{ x: number; y: number }>[] = [];
  const append = (point: Readonly<{ x: number; y: number }>) => {
    const previous = current[current.length - 1];
    if (previous === undefined || Math.hypot(point.x - previous.x, point.y - previous.y) > 1e-8) current.push(point);
  };
  const finish = () => {
    if (current.length > 1) fragments.push(current);
    current = [];
  };
  for (let index = 0; index < points.length - 1; index++) {
    const start = points[index];
    const end = points[index + 1];
    if (start === undefined || end === undefined) continue;
    const clipped = clipMapSegment(start, end, minimumX, maximumX, minimumY, maximumY);
    if (clipped === null) {
      finish();
      continue;
    }
    const previous = current[current.length - 1];
    if (previous !== undefined && Math.hypot(clipped.start.x - previous.x, clipped.start.y - previous.y) > 1e-8) finish();
    append(clipped.start);
    append(clipped.end);
    if (Math.hypot(clipped.end.x - end.x, clipped.end.y - end.y) > 1e-8) finish();
  }
  finish();
  return fragments;
}

function createAnalysisChart(
  analysis: FlightAnalysisData,
  chart: AppModel["analysisChart"],
  cursorSample: AppModel["analysisCursorSample"],
  scenarioId: number | null,
  rect = normalizedRect(0.08, 0.22, 0.84, 0.30)
): UiChart {
  const samples = analysis.samples;
  if (chart === "map") {
    const venue = scenarioId === null ? null : venueMapForScenario(scenarioId);
    const north = samples.map((sample) => sample.northMeters);
    const east = samples.map((sample) => sample.eastMeters);
    const centerNorth = (Math.min(...north) + Math.max(...north)) / 2;
    const centerEast = (Math.min(...east) + Math.max(...east)) / 2;
    const trackSpan = Math.max(Math.max(...north) - Math.min(...north), Math.max(...east) - Math.min(...east), 2);
    const baseHalfRange = trackSpan / 2;
    // Keep the longest trajectory dimension at 75% of the square map viewport.
    const halfRange = baseHalfRange / 0.75;
    const windSpeed = cursorSample === null ? 0 : Math.hypot(
      cursorSample.windNorthMetersPerSecond,
      cursorSample.windEastMetersPerSecond
    );
    const windLengthMeters = baseHalfRange * 0.2;
    const windScale = windSpeed > 0 ? windLengthMeters / windSpeed : 0;
    const vectorEnd = cursorSample === null ? null : {
      x: cursorSample.eastMeters + cursorSample.windEastMetersPerSecond * windScale,
      y: cursorSample.northMeters + cursorSample.windNorthMetersPerSecond * windScale
    };
    const timeMarkers = mapTimeMarkers(samples, analysis.summary.durationSeconds);
    const windGrid = analysis.windGrid;
    const gridMaximumHorizontalWind = windGrid === null || windGrid === undefined
      ? 0
      : Math.max(...windGrid.samples.map((sample) => Math.hypot(
        sample.windNorthMetersPerSecond,
        sample.windEastMetersPerSecond
      )));
    const gridWindScale = gridMaximumHorizontalWind > 0 ? baseHalfRange * 0.12 / gridMaximumHorizontalWind : 0;
    const verticalWindRange = windGrid === null || windGrid === undefined
      ? null
      : [
        Math.min(...windGrid.samples.map((sample) => sample.windDownMetersPerSecond)),
        Math.max(...windGrid.samples.map((sample) => sample.windDownMetersPerSecond))
      ] as const;
    const verticalWindDescription = verticalWindRange === null
      ? "unavailable"
      : `${verticalWindRange[0].toFixed(1)}…${verticalWindRange[1].toFixed(1)}`;
    const series: UiChart["series"][number][] = [{
      label: "飛行軌跡（紫=開始、黄=終端）",
      color: progressColor(0),
      points: samples.map((sample) => ({ x: sample.eastMeters, y: sample.northMeters })),
      segmentColors: samples.slice(0, -1).map((_, index) => progressColor(samples.length <= 2 ? 0.5 : index / (samples.length - 2)))
    }];
    venue?.lines.forEach((line) => {
      const fragments = clipMapPolyline(
        line.points.map((point) => ({ x: point.eastMeters, y: point.northMeters })),
        centerEast - halfRange, centerEast + halfRange, centerNorth - halfRange, centerNorth + halfRange
      );
      fragments.forEach((points, index) => series.push({ label: index === 0 ? line.label : "", color: line.color, points }));
    });
    const clipVector = (start: Readonly<{ x: number; y: number }>, end: Readonly<{ x: number; y: number }>) =>
      clipMapSegment(start, end, centerEast - halfRange, centerEast + halfRange, centerNorth - halfRange, centerNorth + halfRange);
    const selectedWind = windSpeed === 0 || vectorEnd === null
      ? null
      : clipVector({ x: cursorSample?.eastMeters ?? 0, y: cursorSample?.northMeters ?? 0 }, vectorEnd);
    const markers = [
      { label: "Start", color: "#f3f4e8", point: { x: samples[0]?.eastMeters ?? 0, y: samples[0]?.northMeters ?? 0 } },
      {
        label: terminalMarkerLabel(analysis.summary.terminal.reason),
        color: "#f29c78",
        point: { x: samples[samples.length - 1]?.eastMeters ?? 0, y: samples[samples.length - 1]?.northMeters ?? 0 }
      },
      ...(venue?.landmarks.filter((landmark) => landmark.point.eastMeters >= centerEast - halfRange &&
        landmark.point.eastMeters <= centerEast + halfRange && landmark.point.northMeters >= centerNorth - halfRange &&
        landmark.point.northMeters <= centerNorth + halfRange).map((landmark) => ({
        label: landmark.label,
        color: landmark.color,
        point: { x: landmark.point.eastMeters, y: landmark.point.northMeters }
      })) ?? [])
    ];
    const vectors: UiChart["vectors"][number][] = [
      ...(cursorSample === null ? [] : [{
        label: `選択sampleの風 · h ${cursorSample.altitudeMeters.toFixed(1)} m · 矢印は空気の移動先 · ${windScale.toFixed(1)} m/(m/s)`,
        color: "#f2b35e",
        start: selectedWind?.start ?? null,
        end: selectedWind?.end ?? null
      }]),
      ...(windGrid === null || windGrid === undefined ? [] : windGrid.samples.map((sample, index) => {
        const horizontalSpeed = Math.hypot(sample.windNorthMetersPerSecond, sample.windEastMetersPerSecond);
        const hasHorizontalWind = horizontalSpeed > 0 && gridWindScale > 0;
        const clipped = hasHorizontalWind ? clipVector(
          { x: sample.eastMeters, y: sample.northMeters },
          {
            x: sample.eastMeters + sample.windEastMetersPerSecond * gridWindScale,
            y: sample.northMeters + sample.windNorthMetersPerSecond * gridWindScale
          }
        ) : null;
        return {
          label: index === 0
            ? `風断面 h ${windGrid.altitudeMeters.toFixed(1)} m · W_D ${verticalWindDescription} m/s · 矢印は空気の移動先 · 1 m/s = ${gridWindScale.toFixed(1)} m`
            : "",
          color: "#c7eb72",
          start: clipped?.start ?? null,
          end: clipped?.end ?? null
        };
      }))
    ];
    return chartControl("game-analysis-plot", "水平軌跡", "東 E (m)", "北 N (m)", series,
    centerEast - halfRange, centerEast + halfRange, centerNorth - halfRange, centerNorth + halfRange, true,
    null, cursorSample === null ? [] : [{ x: cursorSample.eastMeters, y: cursorSample.northMeters }],
    vectors, markers, timeMarkers, [], rect);
  }
  if (chart === "altitude") {
    const launchAltitude = samples[0]?.altitudeMeters ?? 0;
    const [minimum, maximum] = paddedRange([...samples.map((sample) => sample.altitudeMeters), 0, launchAltitude]);
    return chartControl("game-analysis-plot", "Altitude vs time", "Time (s)", "Altitude (m)", [{
      label: "高度",
      color: "#70d6c8",
      points: samples.map((sample) => ({ x: sample.timeSeconds, y: sample.altitudeMeters }))
    }], 0, Math.max(analysis.summary.durationSeconds, 0.01), minimum, maximum, false,
    modelTimeCursor(analysis, cursorSample), cursorSample === null ? [] : [{ x: cursorSample.timeSeconds, y: cursorSample.altitudeMeters }], [], [], [], [
      { value: 0, label: "静水面", color: "#85c7e8" },
      { value: launchAltitude, label: `発進高度 ${launchAltitude.toFixed(1)} m`, color: "#f2b35e" }
    ], rect);
  }
  const [minimum, maximum] = paddedRange(samples.flatMap((sample) => [sample.airspeedMetersPerSecond, sample.groundspeedMetersPerSecond]));
  return chartControl("game-analysis-plot", "Airspeed / Groundspeed", "Time (s)", "Speed (m/s)", [
    {
      label: "対気速度",
      color: "#70d6c8",
      points: samples.map((sample) => ({ x: sample.timeSeconds, y: sample.airspeedMetersPerSecond }))
    },
    {
      label: "対地速度",
      color: "#f2b35e",
      points: samples.map((sample) => ({ x: sample.timeSeconds, y: sample.groundspeedMetersPerSecond }))
    }
  ], 0, Math.max(analysis.summary.durationSeconds, 0.01), minimum, maximum, false,
  modelTimeCursor(analysis, cursorSample), cursorSample === null ? [] : [
    { x: cursorSample.timeSeconds, y: cursorSample.airspeedMetersPerSecond },
    { x: cursorSample.timeSeconds, y: cursorSample.groundspeedMetersPerSecond }
  ], [], [], [], [], rect);
}

function mapTimeMarkers(
  samples: readonly FlightAnalysisData["samples"][number][],
  durationSeconds: number
): UiChart["timeMarkers"] {
  const intervalSeconds = durationSeconds <= 12 ? 1 : 5;
  const markers: UiChart["timeMarkers"][number][] = [];
  for (let timeSeconds = intervalSeconds; timeSeconds < durationSeconds; timeSeconds += intervalSeconds) {
    let low = 0;
    let high = samples.length - 1;
    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      if ((samples[middle]?.timeSeconds ?? Infinity) < timeSeconds) low = middle + 1;
      else high = middle;
    }
    const sample = samples[low];
    if (sample === undefined) continue;
    markers.push(Object.freeze({
      label: `${String(timeSeconds)} s`,
      color: "#d5e0dc",
      point: Object.freeze({ x: sample.eastMeters, y: sample.northMeters })
    }));
  }
  return Object.freeze(markers);
}

function modelTimeCursor(analysis: FlightAnalysisData, sample: AppModel["analysisCursorSample"]): number | null {
  return sample === null ? null : Math.min(analysis.summary.durationSeconds, sample.timeSeconds);
}

function chartControl(
  id: string,
  label: string,
  xAxisLabel: string,
  yAxisLabel: string,
  series: UiChart["series"],
  xMinimum: number,
  xMaximum: number,
  yMinimum: number,
  yMaximum: number,
  equalAxisScale: boolean,
  cursorX: number | null,
  cursorPoints: UiChart["cursorPoints"],
  vectors: UiChart["vectors"],
  markers: UiChart["markers"],
  timeMarkers: UiChart["timeMarkers"],
  referenceLines: UiChart["referenceLines"],
  rect = normalizedRect(0.08, 0.22, 0.84, 0.30)
): UiChart {
  return Object.freeze({
    kind: "chart",
    id,
    label,
    xAxisLabel,
    yAxisLabel,
    xMinimum,
    xMaximum,
    yMinimum,
    yMaximum,
    equalAxisScale,
    cursorX,
    cursorPoints: Object.freeze(cursorPoints.map((point) => Object.freeze(point))),
    vectors: Object.freeze(vectors.map((vector) => Object.freeze({
      ...vector,
      start: vector.start === null ? null : Object.freeze({ ...vector.start }),
      end: vector.end === null ? null : Object.freeze({ ...vector.end })
    }))),
    markers: Object.freeze(markers.map((marker) => Object.freeze({
      ...marker,
      point: Object.freeze({ ...marker.point })
    }))),
    timeMarkers: Object.freeze(timeMarkers.map((marker) => Object.freeze({
      ...marker,
      point: Object.freeze({ ...marker.point })
    }))),
    referenceLines: Object.freeze(referenceLines.map((line) => Object.freeze({ ...line }))),
    series: Object.freeze(series.map((entry) => Object.freeze({
      ...entry,
      ...(entry.segmentColors === undefined ? {} : { segmentColors: Object.freeze([...entry.segmentColors]) }),
      points: Object.freeze(entry.points.map((point) => Object.freeze(point)))
    }))),
    enabled: false,
    rect
  });
}

function progressColor(progress: number): string {
  const stops = [
    { at: 0, color: [68, 1, 84] },
    { at: 0.5, color: [33, 145, 140] },
    { at: 1, color: [253, 231, 37] }
  ] as const;
  const normalized = Math.min(1, Math.max(0, progress));
  const stopIndex = normalized <= 0.5 ? 0 : 1;
  const start = stops[stopIndex];
  const end = stopIndex === 0 ? stops[1] : stops[2];
  const ratio = (normalized - start.at) / (end.at - start.at);
  const channels = start.color.map((value, index) => {
    const endValue = end.color[index] ?? value;
    return Math.round(value + (endValue - value) * ratio);
  });
  return `#${channels.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

function paddedRange(values: readonly number[]): readonly [number, number] {
  const minimum = Math.min(...values);
  const maximum = Math.max(...values);
  const padding = Math.max((maximum - minimum) * 0.08, Math.abs(maximum) * 0.02, 0.5);
  return [minimum - padding, maximum + padding];
}

function resultAnalysisSummary(analysis: FlightAnalysisData | null): string {
  if (analysis === null) return "FlightRecordを解析中、または解析データを取得できない";
  const { summary } = analysis;
  const score = summary.score === null ? "score unavailable" : `距離 ${summary.score.courseParallelMeters.toFixed(1)} m`;
  const aoa = summary.maximumAngleOfAttackRadians === null
    ? "AoA unavailable"
    : `最大AoA ${(summary.maximumAngleOfAttackRadians * 180 / Math.PI).toFixed(1)}°`;
  return `${terminalMarkerLabel(summary.terminal.reason)} · ${score} · 飛行時間 ${summary.durationSeconds.toFixed(1)} s · 最大対気速度 ${summary.maximumAirspeedMetersPerSecond.toFixed(1)} m/s · ${aoa} · 最大|roll| ${(summary.maximumAbsoluteRollRadians * 180 / Math.PI).toFixed(1)}°`;
}

function terminalMarkerLabel(reason: FlightAnalysisData["summary"]["terminal"]["reason"]): string {
  switch (reason) {
    case "water-contact": return "Splash";
    case "time-limit": return "TimeLimit";
    case "outside-envelope": return "OutsideEnvelope";
    case "manual-abort": return "ManualAbort";
    case "simulation-error": return "SimulationError";
  }
}

function cursorReadout(sample: NonNullable<AppModel["analysisCursorSample"]>): string {
  return `t ${sample.timeSeconds.toFixed(2)} s · N ${sample.northMeters.toFixed(1)} m · E ${sample.eastMeters.toFixed(1)} m · h ${sample.altitudeMeters.toFixed(1)} m\nVair ${sample.airspeedMetersPerSecond.toFixed(1)} · Vground ${sample.groundspeedMetersPerSecond.toFixed(1)} · WN ${sample.windNorthMetersPerSecond.toFixed(1)} · WE ${sample.windEastMetersPerSecond.toFixed(1)} · WD ${sample.windDownMetersPerSecond.toFixed(1)} m/s`;
}

function gameButtons(
  phaseCode: number,
  difficulty: DifficultyUiState,
  configurationMetadata: ConfigurationMetadataUiState | null,
  countdownRemaining: number,
  replayAvailable: boolean,
  canResume: boolean,
  pauseOverlay: "menu" | "settings" | "help" | null
): (UiButton | UiToggle)[] {
  switch (phaseCode) {
    case 0:
      return [button("game-title-start", "飛行を設定", true), button("game-title-demo", "デモ飛行を見る", true)];
    case 1: {
      const setupControls: (UiButton | UiToggle)[] = [
        button("game-setup-preset", `Preset: ${presetLabel(difficulty.presetCode)}`, true),
        button("game-setup-information", `Information: ${informationLabel(difficulty.informationCode)}`, true),
        ...(difficulty.informationCode === 4 ? informationCueToggles(difficulty.hudProfile) : []),
        button("game-setup-assistance", `Assistance: ${assistanceLabel(difficulty.assistanceCode)}`, true),
        button("game-setup-weather", `Weather: ${weatherLabel(difficulty.weatherCode)}`, true),
        button("game-setup-start", "設定を確定してBriefingへ", true),
        button("game-setup-back", "Titleへ戻る", true)
      ];
      return setupControls;
    }
    case 2:
      return [button("game-briefing-cancel", "設定へ戻る", true)];
    case 3:
      return [button("game-briefing-start", "カウントダウン開始", true), button("game-briefing-cancel", "設定へ戻る", true)];
    case 4:
      return [
        button("game-countdown-status", `発進まで ${String(countdownRemaining)}`, false),
        button("game-countdown-cancel", "発進を取消", true)
      ];
    case 5:
      return [
        button("game-flight-pause", "Pause", true),
        button("game-flight-abort", "飛行を終了", true)
      ];
    case 6:
      if (pauseOverlay === "settings") return [button("game-pause-settings-back", "Pauseへ戻る", true)];
      if (pauseOverlay === "help") return [button("game-pause-help-back", "Pauseへ戻る", true)];
      return [
        button("game-flight-resume", "Resume", canResume),
        button("game-pause-open-settings", "Settings", true),
        button("game-pause-open-help", "Help", true),
        button("game-paused-abort", "飛行を終了", true)
      ];
    case 7:
      return [
        button("game-result-open-analysis", "Analysisを見る", true),
        button("game-result-configuration", configurationSummary(configurationMetadata), false),
        button("game-result-replay", "Replayを見る", replayAvailable),
        button("game-result-retry", "同じ条件で再試行", true),
        button("game-result-setup", "設定を変更", true),
        button("game-result-title", "Titleへ戻る", true)
      ];
    case 8:
      return [
        button("game-briefing-retry", "準備を再試行", true),
        button("game-failed-setup", "設定へ戻る", true)
      ];
    case 9:
      return [button("game-replay-return", "Resultへ戻る", true)];
    case 10:
      return [button("game-attract-return", "Titleへ戻る", true)];
    default:
      return [];
  }
}

function configurationSummary(metadata: ConfigurationMetadataUiState | null): string {
  if (metadata === null) return "解決済み設定metadataを取得できない";
  const axes = `${presetLabel(metadata.presetCode)} / ${informationLabel(metadata.informationCode)} / ${assistanceLabel(metadata.assistanceCode)} / ${weatherLabel(metadata.weatherCode)}`;
  const cues = metadata.informationCode === 4
    ? ` · HUD ${[
      metadata.hudProfile.telemetry ? "Telemetry" : null,
      metadata.hudProfile.attitude ? "Attitude" : null,
      metadata.hudProfile.wind ? "Wind" : null,
      metadata.hudProfile.flightPath ? "Flight path" : null,
      metadata.hudProfile.angleOfAttack ? "AoA" : null,
      metadata.hudProfile.warnings ? "Warnings" : null
    ].filter((cue) => cue !== null).join(", ") || "none"}`
    : "";
  const model = [
    `Catalog v${String(metadata.catalogVersion)}`,
    `Scenario ${String(metadata.scenarioId)} v${String(metadata.scenarioVersion)}`,
    `Aircraft v${String(metadata.aircraftModelVersion)}`,
    `Environment v${String(metadata.environmentVersion)}`,
    `Controller v${String(metadata.controllerProfileVersion)}`,
    `Seed ${String(metadata.seedHigh)}:${String(metadata.seedLow)}`
  ].join(" · ");
  return `${axes}${cues}\n${model}`;
}

function button(id: string, label: string, enabled: boolean): UiButton {
  return Object.freeze({ kind: "button", id, label, enabled, rect: normalizedRect(0.08, 0.72, 0.84, 0.075) });
}

function informationCueToggles(profile: DifficultyUiState["hudProfile"]): UiToggle[] {
  return [
    toggle("game-setup-information-telemetry", "速度・高度・距離・時間", profile.telemetry),
    toggle("game-setup-information-attitude", "姿勢・方位", profile.attitude),
    toggle("game-setup-information-wind", "風", profile.wind),
    toggle("game-setup-information-flight-path", "飛行経路指示", profile.flightPath),
    toggle("game-setup-information-angle-of-attack", "迎角", profile.angleOfAttack),
    toggle("game-setup-information-warnings", "警告cue", profile.warnings)
  ];
}

function toggle(id: string, label: string, value: boolean): UiToggle {
  return Object.freeze({ kind: "toggle", id, label, value, enabled: true, rect: normalizedRect(0.08, 0.72, 0.84, 0.075) });
}

function formatSavedAt(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? new Date(timestamp).toLocaleString() : "日時不明";
}

function replayCameraLabel(mode: AppModel["replayCameraMode"]): string {
  return mode.charAt(0).toUpperCase() + mode.slice(1);
}

function status(id: string, label: string, value: string): UiStatus {
  return Object.freeze({ kind: "status", id, label, value, enabled: false, rect: normalizedRect(0.08, 0.84, 0.84, 0.075) });
}

function sceneForPhase(phaseCode: number): UiViewModel["scene"] {
  switch (phaseCode) {
    case 0: return "Title";
    case 1: return "FlightSetup";
    case 2:
    case 3:
    case 8: return "Briefing";
    case 4: return "Countdown";
    case 5:
    case 6: return "Flight";
    case 7: return "Result";
    case 9: return "Replay";
    case 10: return "Title";
    default: return "Boot";
  }
}

function titleForScene(scene: UiViewModel["scene"]): string {
  switch (scene) {
    case "Title": return "鳥人間滑空ゲーム";
    case "FlightSetup": return "Flight Setup";
    case "Briefing": return "Briefing";
    case "Countdown": return "発進準備";
    case "Flight": return "Flight";
    case "Result": return "Flight Result";
    case "Replay": return "Flight Replay";
    case "Boot": return "鳥人間滑空ゲーム";
  }
}

function descriptionForPhase(phaseCode: number, difficulty: DifficultyUiState, countdownRemaining: number, canResume = false): string {
  switch (phaseCode) {
    case 0: return "合成scenarioを用いて滑空飛行を行う。";
    case 1: return `合成scenarioを使用する。${assistanceLabel(difficulty.assistanceCode)}、${weatherLabel(difficulty.weatherCode)}。`;
    case 2: return "飛行設定と必要assetを準備している。";
    case 3: return "設定を固定した。発進操作でCountdownを開始する。";
    case 4: return `物理時間を停止している。発進まで ${String(countdownRemaining)}。`;
    case 5: return "KeyboardまたはGamepadで操縦する。Physicsは100 Hzで独立して進む。";
    case 6: return canResume
      ? "飛行状態を保持して停止している。Resumeで再開できる。"
      : "飛行状態を保持して停止している。外部の一時停止条件が解消するまで再開できない。";
    case 7: return "確定済みterminal snapshotから結果を表示する。";
    case 8: return "Briefing準備に失敗した。再試行または設定変更を選択する。";
    case 9: return "確定済みFlightRecordのposeを再生している。physics stateは進行しない。";
    case 10: return "Title用の独立したFlightRecordを自動再生している。";
    default: return "表示基盤を初期化している。";
  }
}

function presetLabel(code: number): string {
  switch (code) {
    case 0: return "Beginner";
    case 1: return "Standard";
    case 2: return "Expert";
    case 3: return "Realistic";
    case 4: return "Custom";
    default: return "不明";
  }
}

function informationLabel(code: number): string {
  switch (code) {
    case 0: return "Full";
    case 1: return "Standard";
    case 2: return "Minimal";
    case 3: return "Realistic";
    case 4: return "Custom";
    default: return "不明";
  }
}

function assistanceLabel(code: number): string {
  switch (code) {
    case 0: return "Strong / Automatic FBW";
    case 1: return "Assisted / Shared FBW";
    case 2: return "Light / Shared FBW";
    case 3: return "Manual";
    default: return "不明";
  }
}

function weatherLabel(code: number): string {
  switch (code) {
    case 0: return "Synthetic Calm";
    case 1: return "Synthetic Mild";
    case 2: return "Synthetic Typical";
    case 3: return "Synthetic Challenging";
    case 4: return "Synthetic NearLimit";
    default: return "不明";
  }
}

function activeModeOf(state: PresentationUiState): "screen" | "webxr" | "phone-vr" | null {
  return state.type === "ready" ? state.mode : null;
}

function canRequest(state: PresentationUiState): boolean {
  return state.type === "failed" || (state.type === "ready" && state.mode === "screen");
}
