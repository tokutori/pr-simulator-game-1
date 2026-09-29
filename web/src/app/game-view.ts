import { IDENTITY_POSE } from "../render/contracts/math.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiButton, UiChart, UiPanel, UiRange, UiStatus, UiViewModel } from "../render/contracts/ui.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightAnalysisData } from "../game/flight-record-query.js";
import { syntheticVenueMapForScenario } from "../game/synthetic-venue-map.js";
import type {
  AppModel,
  ConfigurationMetadataUiState,
  DifficultyUiState,
  PresentationUiState
} from "./app-state.js";
import { gameSessionCountdown, gameSessionPhaseCode } from "./app-state.js";

export function createGameViewModel(
  model: AppModel,
  _snapshot: FlightSnapshot | null,
  analysis: FlightAnalysisData | null = model.flightAnalysis
): UiViewModel {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  const countdownRemaining = gameSessionCountdown(model.gameSession);
  const scene = sceneForPhase(phaseCode);
  const buttons = gameButtons(
    phaseCode,
    model.difficulty,
    model.configurationMetadata,
    countdownRemaining,
    model.flightAnalysis !== null
  );
  if (phaseCode === 0) {
    const savedFlightButtons = model.storedFlightRecords.slice(0, 3).map((record) => button(
      `game-title-open-record-${String(record.id)}`,
      `Replay #${String(record.id)} · ${formatSavedAt(record.savedAt)}`,
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

  const controls: (UiButton | UiRange | UiStatus | UiChart)[] = phaseCode === 5
    ? []
    : [status("game-state", "状態", model.status || descriptionForPhase(phaseCode, model.difficulty, countdownRemaining))];
  if (phaseCode === 7 && model.resultTab === "summary") {
    const result = status("game-result-analysis", "Summary", resultAnalysisSummary(analysis));
    controls.push(Object.freeze({ ...result, rect: normalizedRect(0.08, 0.15, 0.84, 0.085) }));
  }
  if (phaseCode === 7 && model.resultTab === "analysis") {
    controls.push(
      Object.freeze({ ...button("game-result-open-summary", "Summaryへ戻る", true), rect: normalizedRect(0.08, 0.79, 0.84, 0.07) }),
      Object.freeze({ ...button("game-analysis-map", model.analysisChart === "map" ? "● Map" : "Map", true), rect: normalizedRect(0.08, 0.70, 0.25, 0.07) }),
      Object.freeze({ ...button("game-analysis-altitude", model.analysisChart === "altitude" ? "● Altitude" : "Altitude", true), rect: normalizedRect(0.375, 0.70, 0.25, 0.07) }),
      Object.freeze({ ...button("game-analysis-speed", model.analysisChart === "speed" ? "● Speed" : "Speed", true), rect: normalizedRect(0.67, 0.70, 0.25, 0.07) })
    );
    if (analysis !== null) controls.push(Object.freeze({
      ...button("game-result-replay", "Replayを見る", true),
      rect: normalizedRect(0.08, 0.11, 0.84, 0.07)
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
      rect: normalizedRect(0.08, 0.625, 0.84, 0.055)
    }));
    if (model.analysisCursorSample !== null) {
      controls.push(Object.freeze({
        ...status("game-analysis-cursor-values", "At cursor", cursorReadout(model.analysisCursorSample)),
        rect: normalizedRect(0.08, 0.555, 0.84, 0.06)
      }));
    }
    if (analysis === null) {
      controls.push(Object.freeze({ ...status("game-analysis-loading", "Analysis", "FlightRecordを取得している"), rect: normalizedRect(0.08, 0.22, 0.84, 0.42) }));
    } else {
      controls.push(createAnalysisChart(
        analysis,
        model.analysisChart,
        model.analysisCursorSample,
        model.configurationMetadata?.scenarioId ?? null
      ));
    }
  } else if (phaseCode === 9) {
    const durationSeconds = analysis?.summary.durationSeconds ?? 0;
    controls.push(Object.freeze({
      ...status("game-replay-time", "Record time", `${model.analysisCursorTimeSeconds.toFixed(2)} / ${durationSeconds.toFixed(2)} s`),
      rect: normalizedRect(0.08, 0.62, 0.84, 0.08)
    }));
    if (analysis !== null) controls.push(Object.freeze({
      kind: "range",
      id: "game-replay-cursor",
      label: "Replay time (s)",
      value: model.analysisCursorTimeSeconds,
      minimum: 0,
      maximum: Math.max(durationSeconds, 0.01),
      step: 0.01,
      enabled: true,
      rect: normalizedRect(0.08, 0.52, 0.84, 0.07)
    }));
    if (model.pendingReplayPoseRequestId !== null || model.replayPose === null) {
      controls.push(Object.freeze({
        ...status("game-replay-pose-status", "Replay", "記録済みposeを取得している"),
        rect: normalizedRect(0.08, 0.40, 0.84, 0.08)
      }));
    }
    controls.push(
      Object.freeze({
        ...button("game-replay-play-pause", model.replayPlaying ? "Pause" : "Play", analysis !== null),
        rect: normalizedRect(0.08, 0.31, 0.30, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-camera", `Camera: ${replayCameraLabel(model.replayCameraMode)}`, model.presentation.type === "ready" && model.presentation.mode === "screen"),
        rect: normalizedRect(0.40, 0.31, 0.52, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-speed-0_5", model.replaySpeed === 0.5 ? "● 0.5×" : "0.5×", true),
        rect: normalizedRect(0.08, 0.22, 0.25, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-speed-1", model.replaySpeed === 1 ? "● 1×" : "1×", true),
        rect: normalizedRect(0.375, 0.22, 0.25, 0.07)
      }),
      Object.freeze({
        ...button("game-replay-speed-2", model.replaySpeed === 2 ? "● 2×" : "2×", true),
        rect: normalizedRect(0.67, 0.22, 0.25, 0.07)
      })
    );
    buttons.forEach((entry) => controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.08, 0.12, 0.84, 0.075) })));
  } else if (phaseCode === 10) {
    controls.push(
      Object.freeze({ ...status("game-attract-status", "Demo", model.status || "自動再生中"), rect: normalizedRect(0.08, 0.72, 0.84, 0.08) }),
      Object.freeze({ ...button("game-attract-return", "Titleへ戻る", true), rect: normalizedRect(0.08, 0.60, 0.84, 0.075) })
    );
  } else {
    let bottom = 0.795;
    buttons.forEach((entry) => {
      const height = entry.id === "game-result-configuration" ? 0.11 : 0.075;
      const y = bottom - height;
      if (y < 0.035) return;
      controls.push(Object.freeze({ ...entry, rect: normalizedRect(0.08, y, 0.84, height) }));
      bottom = y - 0.01;
    });
  }
  const panel: UiPanel = Object.freeze({
    id: "game-flow",
    title: "ゲーム進行",
    anchor: "menu",
    localPose: IDENTITY_POSE,
    size: Object.freeze({ width: 2.4, height: 1.8 }),
    controls: Object.freeze(controls)
  });
  return Object.freeze({
    scene,
    title: titleForScene(scene),
    description: phaseCode === 0
      ? [descriptionForPhase(phaseCode, model.difficulty, countdownRemaining),
        model.storedFlightRecordsStatus,
        model.storedFlightRecords.length > 0 ? `保存FlightRecord ${String(model.storedFlightRecords.length)}件` : ""]
        .filter(Boolean).join(" · ")
      : descriptionForPhase(phaseCode, model.difficulty, countdownRemaining),
    activeOverlay: phaseCode === 6 ? "Pause" : null,
    panels: Object.freeze([panel])
  });
}

function createAnalysisChart(
  analysis: FlightAnalysisData,
  chart: AppModel["analysisChart"],
  cursorSample: AppModel["analysisCursorSample"],
  scenarioId: number | null
): UiChart {
  const samples = analysis.samples;
  if (chart === "map") {
    const venue = scenarioId === null ? null : syntheticVenueMapForScenario(scenarioId);
    const venuePoints = venue === null ? [] : [
      ...venue.lines.flatMap((line) => line.points),
      ...venue.landmarks.map((landmark) => landmark.point)
    ];
    const north = [...samples.map((sample) => sample.northMeters), ...venuePoints.map((point) => point.northMeters)];
    const east = [...samples.map((sample) => sample.eastMeters), ...venuePoints.map((point) => point.eastMeters)];
    const centerNorth = (Math.min(...north) + Math.max(...north)) / 2;
    const centerEast = (Math.min(...east) + Math.max(...east)) / 2;
    const baseHalfRange = Math.max(Math.max(...north) - Math.min(...north), Math.max(...east) - Math.min(...east), 2) / 2;
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
    const halfRange = vectorEnd === null
      ? baseHalfRange
      : Math.max(baseHalfRange, Math.abs(vectorEnd.x - centerEast), Math.abs(vectorEnd.y - centerNorth)) / 0.9;
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
    venue?.lines.forEach((line) => series.push({
      label: line.label,
      color: line.color,
      points: line.points.map((point) => ({ x: point.eastMeters, y: point.northMeters }))
    }));
    const markers = [
      { label: "Start", color: "#f3f4e8", point: { x: samples[0]?.eastMeters ?? 0, y: samples[0]?.northMeters ?? 0 } },
      {
        label: terminalMarkerLabel(analysis.summary.terminal.reason),
        color: "#f29c78",
        point: { x: samples[samples.length - 1]?.eastMeters ?? 0, y: samples[samples.length - 1]?.northMeters ?? 0 }
      },
      ...(venue?.landmarks.map((landmark) => ({
        label: landmark.label,
        color: landmark.color,
        point: { x: landmark.point.eastMeters, y: landmark.point.northMeters }
      })) ?? [])
    ];
    const vectors: UiChart["vectors"][number][] = [
      ...(cursorSample === null ? [] : [{
        label: `選択sampleの風 · h ${cursorSample.altitudeMeters.toFixed(1)} m · 矢印は空気の移動先 · ${windScale.toFixed(1)} m/(m/s)`,
        color: "#f2b35e",
        start: windSpeed === 0 || vectorEnd === null ? null : { x: cursorSample.eastMeters, y: cursorSample.northMeters },
        end: windSpeed === 0 ? null : vectorEnd
      }]),
      ...(windGrid === null || windGrid === undefined ? [] : windGrid.samples.map((sample, index) => {
        const horizontalSpeed = Math.hypot(sample.windNorthMetersPerSecond, sample.windEastMetersPerSecond);
        const hasHorizontalWind = horizontalSpeed > 0 && gridWindScale > 0;
        return {
          label: index === 0
            ? `風断面 h ${windGrid.altitudeMeters.toFixed(1)} m · W_D ${verticalWindDescription} m/s · 矢印は空気の移動先 · 1 m/s = ${gridWindScale.toFixed(1)} m`
            : "",
          color: "#c7eb72",
          start: hasHorizontalWind ? { x: sample.eastMeters, y: sample.northMeters } : null,
          end: hasHorizontalWind ? {
            x: sample.eastMeters + sample.windEastMetersPerSecond * gridWindScale,
            y: sample.northMeters + sample.windNorthMetersPerSecond * gridWindScale
          } : null
        };
      }))
    ];
    return chartControl("game-analysis-plot", "水平軌跡", "東 E (m)", "北 N (m)", series,
    centerEast - halfRange, centerEast + halfRange, centerNorth - halfRange, centerNorth + halfRange, true,
    null, cursorSample === null ? [] : [{ x: cursorSample.eastMeters, y: cursorSample.northMeters }],
    vectors, markers, timeMarkers, []);
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
    ]);
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
  ], [], [], [], []);
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
  referenceLines: UiChart["referenceLines"]
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
    rect: normalizedRect(0.08, 0.22, 0.84, 0.30)
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
  replayAvailable: boolean
): UiButton[] {
  switch (phaseCode) {
    case 0:
      return [button("game-title-start", "飛行を設定", true), button("game-title-demo", "デモ飛行を見る", true)];
    case 1:
      return [
        button("game-setup-preset", `Preset: ${presetLabel(difficulty.presetCode)}`, true),
        button("game-setup-information", `Information: ${informationLabel(difficulty.informationCode)}`, true),
        button("game-setup-assistance", `Assistance: ${assistanceLabel(difficulty.assistanceCode)}`, true),
        button("game-setup-weather", `Weather: ${weatherLabel(difficulty.weatherCode)}`, true),
        button("game-setup-start", "設定を確定してBriefingへ", true),
        button("game-setup-back", "Titleへ戻る", true)
      ];
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
      return [
        button("game-flight-resume", "Resume", true),
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
  const model = [
    `Catalog v${String(metadata.catalogVersion)}`,
    `Scenario ${String(metadata.scenarioId)} v${String(metadata.scenarioVersion)}`,
    `Aircraft v${String(metadata.aircraftModelVersion)}`,
    `Environment v${String(metadata.environmentVersion)}`,
    `Controller v${String(metadata.controllerProfileVersion)}`,
    `Seed ${String(metadata.seedHigh)}:${String(metadata.seedLow)}`
  ].join(" · ");
  return `${axes}\n${model}`;
}

function button(id: string, label: string, enabled: boolean): UiButton {
  return Object.freeze({ kind: "button", id, label, enabled, rect: normalizedRect(0.08, 0.72, 0.84, 0.075) });
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

function descriptionForPhase(phaseCode: number, difficulty: DifficultyUiState, countdownRemaining: number): string {
  switch (phaseCode) {
    case 0: return "合成scenarioを用いて滑空飛行を行う。";
    case 1: return `合成scenarioを使用する。${assistanceLabel(difficulty.assistanceCode)}、${weatherLabel(difficulty.weatherCode)}。`;
    case 2: return "飛行設定と必要assetを準備している。";
    case 3: return "設定を固定した。発進操作でCountdownを開始する。";
    case 4: return `物理時間を停止している。発進まで ${String(countdownRemaining)}。`;
    case 5: return "KeyboardまたはGamepadで操縦する。Physicsは100 Hzで独立して進む。";
    case 6: return "飛行状態を保持して停止している。Resumeで再開する。";
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
