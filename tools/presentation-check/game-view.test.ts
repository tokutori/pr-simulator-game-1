import { describe, expect, it } from "vitest";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import { drawVrPanel } from "../../web/src/presentation/vr-panel-canvas.js";
import type { PanelDrawingContext } from "../../web/src/presentation/vr-panel-canvas.js";
import { chartScaleBarDistance, fitPlotRectToEqualScale, formatChartTick, validateUiViewModel } from "../../web/src/render/contracts/ui.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const flightSnapshotValues = Array.from({ length: 33 }, () => 0);
flightSnapshotValues[7] = 1;
flightSnapshotValues[19] = -1;
const flightSnapshot = parseFlightSnapshot(flightSnapshotValues);

describe("Game scene view model", () => {
  it.each([
    [0, "Title"],
    [1, "FlightSetup"],
    [2, "Briefing"],
    [3, "Briefing"],
    [4, "Countdown"],
    [5, "Flight"],
    [6, "Flight"],
    [7, "Result"],
    [8, "Briefing"],
    [10, "Title"]
  ] as const)("maps phase %i to %s", (gamePhaseCode, scene) => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(gamePhaseCode) });
    const view = createGameViewModel(model, null);
    expect(view.scene).toBe(scene);
    const panel = view.panels[0];
    expect(panel?.anchor).toBe("menu");
    const controlIds = panel?.controls.map((control) => control.id) ?? [];
    expect(new Set(controlIds).size).toBe(controlIds.length);
  });

  it("keeps Replay explicitly unavailable until the recording milestone", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(7) });
    const view = createGameViewModel(model, null);
    const replay = view.panels[0]?.controls.find((control) => control.id === "game-result-replay");
    expect(replay).toMatchObject({ kind: "button", enabled: false });
  });

  it("keeps Flight free of the centered status and metrics panel", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(5) });
    const controls = createGameViewModel(model, null).panels[0]?.controls ?? [];
    expect(controls.map((control) => control.id)).toEqual(["game-flight-pause", "game-flight-abort"]);
    expect(controls.some((control) => control.id === "game-state" || control.id === "game-flight-metrics")).toBe(false);
  });

  it("enables Resume only when Rust reports that pause causes permit resuming", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(6) });
    const blocked = createGameViewModel(model, flightSnapshot).panels[0]?.controls
      .find((control) => control.id === "game-flight-resume");
    expect(blocked).toMatchObject({ kind: "button", enabled: false });

    const availableModel = Object.freeze({
      ...model,
      gameSession: sessionForPhase(6, true)
    });
    const available = createGameViewModel(availableModel, flightSnapshot).panels[0]?.controls
      .find((control) => control.id === "game-flight-resume");
    expect(available).toMatchObject({ kind: "button", enabled: true });
  });

  it("projects separate Pause, Settings, and Help overlays while the flight remains paused", () => {
    const menuModel = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(6) });
    const menu = createGameViewModel(menuModel, flightSnapshot);
    expect(menu.activeOverlay).toBe("Pause");
    expect(menu.panels[0]?.controls.map((control) => control.id)).toEqual(expect.arrayContaining([
      "game-flight-resume", "game-pause-open-settings", "game-pause-open-help", "game-paused-abort"
    ]));

    const settingsModel = Object.freeze({
      ...menuModel,
      gameSession: { ...menuModel.gameSession, overlay: { kind: "settings" as const } }
    });
    const settings = createGameViewModel(settingsModel, flightSnapshot);
    expect(settings.activeOverlay).toBe("PauseSettings");
    expect(settings.panels[0]?.controls.map((control) => control.id)).toEqual(expect.arrayContaining([
      "game-pause-settings-info", "game-pause-settings-back"
    ]));
    expect(settings.panels[0]?.controls.some((control) => control.id === "game-flight-resume")).toBe(false);

    const helpModel = Object.freeze({
      ...menuModel,
      gameSession: { ...menuModel.gameSession, overlay: { kind: "help" as const } }
    });
    const help = createGameViewModel(helpModel, flightSnapshot);
    expect(help.activeOverlay).toBe("PauseHelp");
    expect(help.panels[0]?.controls.map((control) => control.id)).toEqual(expect.arrayContaining([
      "game-pause-help-info", "game-pause-help-back"
    ]));
    expect(help.panels[0]?.controls.some((control) => control.id === "game-flight-resume")).toBe(false);

    const vrModel = Object.freeze({
      ...helpModel,
      presentation: { type: "ready" as const, mode: "webxr" as const }
    });
    const vrHelp = createGameViewModel(vrModel, flightSnapshot);
    expect(vrHelp.panels[0]?.anchor).toBe("menu");
    expect(vrHelp.panels[0]?.controls.map((control) => control.id)).toEqual(expect.arrayContaining([
      "game-pause-help-info", "game-pause-help-back"
    ]));
    expect(vrHelp.panels[0]?.controls.every((control) =>
      control.rect.x >= 0 && control.rect.y >= 0
      && control.rect.x + control.rect.width <= 1
      && control.rect.y + control.rect.height <= 1
    )).toBe(true);
  });

  it("renders one Menu-anchored Pause control separately from VR Flight information", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(5),
      presentation: Object.freeze({ type: "ready" as const, mode: "webxr" as const })
    });
    const panel = createGameViewModel(model, flightSnapshot).panels[0];
    if (panel === undefined) throw new Error("Flight panel is missing");
    const controlIds = panel.controls.map((control) => control.id);
    expect(panel.anchor).toBe("menu");
    expect(controlIds).not.toContain("game-flight-readouts");
    expect(controlIds).toContain("game-flight-pause");
    expect(controlIds).not.toContain("game-flight-abort");
    expect(new Set(controlIds).size).toBe(controlIds.length);
  });

  it("lists recent persisted records in Title", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(0),
      storedFlightRecords: Object.freeze([
        Object.freeze({ id: 12, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }),
        Object.freeze({ id: 11, savedAt: "2026-09-27T00:00:00.000Z", personalBest: false })
      ])
    });
    const controls = createGameViewModel(model, null).panels[0]?.controls;
    expect(controls?.find((control) => control.id === "game-title-open-record-12"))
      .toMatchObject({ kind: "button", enabled: true });
    expect(controls?.find((control) => control.id === "game-title-open-record-11"))
      .toMatchObject({ kind: "button", enabled: true });
    expect(controls?.find((control) => control.id === "game-title-open-record-12")?.label)
      .toContain("Personal Best");
    expect(controls?.filter((control) => control.id.startsWith("game-title-open-record-"))).toHaveLength(2);
    const pendingControls = createGameViewModel({ ...model, pendingGameRequestId: 2 }, null).panels[0]?.controls;
    expect(pendingControls?.find((control) => control.id === "game-title-open-record-12"))
      .toMatchObject({ kind: "button", enabled: false });
    expect(pendingControls?.find((control) => control.id === "game-title-start"))
      .toMatchObject({ kind: "button", enabled: false });
    expect(pendingControls?.find((control) => control.id === "game-title-demo"))
      .toMatchObject({ kind: "button", enabled: false });
  });

  it("disables Result and Analysis actions while a game operation is pending", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(7),
      resultTab: "analysis" as const,
      pendingGameRequestId: 2
    });
    const controls = createGameViewModel(model, null).panels[0]?.controls ?? [];
    const buttons = controls.filter((control) => control.kind === "button");
    expect(buttons.length).toBeGreaterThan(0);
    expect(buttons.every((control) => !control.enabled)).toBe(true);
  });

  it("shows the Attract return action while the Title demo is playing", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(10), status: "" });
    const controls = createGameViewModel(model, null).panels[0]?.controls;
    expect(controls?.find((control) => control.id === "game-attract-status"))
      .toMatchObject({ kind: "status", value: "自動再生中" });
    expect(controls?.find((control) => control.id === "game-attract-return"))
      .toMatchObject({ kind: "button", enabled: true });
  });

  it("projects the Rust Replay phase with a shared record-time scrubber", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(9),
      replayViewMode: "telemetry" as const,
      analysisCursorTimeSeconds: 1.25,
      flightAnalysis: Object.freeze({
        samples: Object.freeze([]),
        initialPilotPositionMeters: 0.1,
        summary: Object.freeze({
          sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
          maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
          maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
          terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
        })
      })
    });
    const view = createGameViewModel(model, null);
    expect(view.scene).toBe("Replay");
    expect(view.panels[0]?.controls.find((control) => control.id === "game-replay-cursor"))
      .toMatchObject({ kind: "range", value: 1.25, maximum: 2 });
    expect(view.panels[0]?.controls.find((control) => control.id === "game-replay-return"))
      .toMatchObject({ kind: "button", enabled: true });
  });

  it("exposes camera selection for Screen and VR Replay", () => {
    const base = {
      ...createInitialAppModel(),
      gameSession: sessionForPhase(9),
      flightAnalysis: Object.freeze({
        samples: Object.freeze([]),
        initialPilotPositionMeters: 0,
        summary: Object.freeze({
          sampleCount: 2, durationSeconds: 1, maximumAltitudeMeters: 1,
          maximumAirspeedMetersPerSecond: 1, maximumGroundspeedMetersPerSecond: 1,
          maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
          terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 1 })
        })
      })
    };
    const screenView = createGameViewModel({
      ...base,
      presentation: Object.freeze({ type: "ready", mode: "screen" })
    }, null);
    expect(screenView.panels[0]?.controls.find((control) => control.id === "game-replay-camera"))
      .toMatchObject({ kind: "button", label: "Camera: Auto", enabled: true });
    const xrView = createGameViewModel({
      ...base,
      presentation: Object.freeze({ type: "ready", mode: "webxr" })
    }, null);
    expect(xrView.panels[0]?.controls.find((control) => control.id === "game-replay-camera"))
      .toMatchObject({ kind: "button", enabled: true });
  });

  it("shows all three selected axes in Setup", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(1),
      difficulty: Object.freeze({
        presetCode: 4, informationCode: 2, hudProfile: createInitialAppModel().difficulty.hudProfile,
        assistanceCode: 0, weatherCode: 3
      })
    });
    const controls = createGameViewModel(model, null).panels[0]?.controls;
    expect(controls?.find((control) => control.id === "game-setup-preset-current")).toMatchObject({ value: "Custom · プリセットから変更済み" });
    expect(controls?.find((control) => control.id === "game-setup-select-information-2")).toMatchObject({ label: "Minimal", presentation: { kind: "choice", selected: true } });
    expect(controls?.find((control) => control.id === "game-setup-select-assistance-0")).toMatchObject({ label: "Strong / Automatic FBW", presentation: { kind: "choice", selected: true } });
    expect(controls?.find((control) => control.id === "game-setup-select-weather-3")).toMatchObject({ label: "Synthetic Challenging", presentation: { kind: "choice", selected: true } });
  });

  it("shows independently configured HUD cues only for Custom Information", () => {
    const base = createInitialAppModel();
    const model = Object.freeze({
      ...base,
      gameSession: sessionForPhase(1),
      difficulty: Object.freeze({
        ...base.difficulty,
        informationCode: 4,
        hudProfile: Object.freeze({ ...base.difficulty.hudProfile, wind: false })
      })
    });
    const controls = createGameViewModel(model, null).panels[0]?.controls;
    expect(controls?.filter((control) => control.kind === "toggle")).toMatchObject([
      { id: "game-setup-information-telemetry", value: true },
      { id: "game-setup-information-attitude", value: true },
      { id: "game-setup-information-wind", value: false },
      { id: "game-setup-information-flight-path", value: true },
      { id: "game-setup-information-angle-of-attack", value: true },
      { id: "game-setup-information-warnings", value: true }
    ]);
  });

  it("shows resolved axes and model versions in Result", () => {
    const model = Object.freeze({
      ...createInitialAppModel(),
      gameSession: sessionForPhase(7),
      configurationMetadata: Object.freeze({
        presetCode: 4, informationCode: 2, hudProfile: createInitialAppModel().difficulty.hudProfile,
        assistanceCode: 3, weatherCode: 4,
        catalogVersion: 1, scenarioId: 5, scenarioVersion: 1, aircraftModelVersion: 1,
        environmentVersion: 5, controllerProfileVersion: 4, seedLow: 0, seedHigh: 0
      })
    });
    const configuration = createGameViewModel(model, null).panels[0]?.controls.find((control) => control.id === "game-result-configuration");
    expect(configuration?.label).toContain("Custom / Minimal / Manual / Synthetic NearLimit");
    expect(configuration?.label).toContain("Catalog v1");
    expect(configuration?.label).toContain("Scenario 5 v1");
    expect(configuration?.label).toContain("Controller v4");
    expect(configuration?.label).toContain("Seed 0:0");
    expect(configuration?.label).toContain("\n");
    expect(configuration?.rect.height).toBe(0.11);
  });

  it("shows Rust-derived FlightRecord summary metrics in Result", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(7) });
    const analysis = Object.freeze({
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 101, durationSeconds: 1, maximumAltitudeMeters: 11,
        maximumAirspeedMetersPerSecond: 12, maximumGroundspeedMetersPerSecond: 13,
        maximumAngleOfAttackRadians: 0.2, maximumAbsoluteRollRadians: 0.1,
        score: Object.freeze({ courseParallelMeters: 24, crossTrackMeters: -2, netHorizontalMeters: 24.1 }),
        terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 1 })
      })
    });
    const result = createGameViewModel(model, null, analysis);
    const summary = result.panels[0]?.controls.find((control) => control.id === "game-result-analysis");
    if (summary?.kind !== "status") throw new Error("Result Analysis summary status is missing");
    expect(summary.label).toBe("Summary");
    expect(summary.value).toContain("距離 24.0 m");
    expect(summary.value).toContain("最大対気速度 12.0 m/s");
  });

  it.each(["map", "altitude", "speed"] as const)("builds a valid %s chart from recorded samples", (analysisChart) => {
    const base = createInitialAppModel();
    const sample = (timeSeconds: number, northMeters: number, eastMeters: number, altitudeMeters: number, airspeedMetersPerSecond: number, groundspeedMetersPerSecond: number) => Object.freeze({
      timeSeconds, northMeters, eastMeters, altitudeMeters,
      airspeedMetersPerSecond, groundspeedMetersPerSecond,
      windNorthMetersPerSecond: 2, windEastMetersPerSecond: -1, windDownMetersPerSecond: 0,
      angleOfAttackRadians: null, sideslipRadians: null,
      rollRadians: 0, pitchRadians: 0, headingRadians: 0
    });
    const analysis = Object.freeze({
      samples: Object.freeze([sample(0, 0, 0, 10, 9, 10), sample(1, 8, 6, 8, 8, 9)]),
      initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 1, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0,
        score: null,
        terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 1 })
      })
    });
    const model = Object.freeze({
      ...base, gameSession: sessionForPhase(7), resultTab: "analysis" as const, analysisChart, flightAnalysis: analysis,
      analysisCursorSample: analysis.samples[1] ?? null
    });
    const view = createGameViewModel(model, null);
    validateUiViewModel(view);
    if (analysisChart === "map") {
      expect(view.panels[0]?.controls.find((control) => control.id === "game-result-replay"))
        .toMatchObject({ kind: "button", enabled: true });
    }
    const chart = view.panels[0]?.controls.find((control) => control.kind === "chart");
    const cursorValues = view.panels[0]?.controls.find((control) => control.id === "game-analysis-cursor-values");
    expect(chart).toMatchObject({ kind: "chart", enabled: false });
    const controls = view.panels[0]?.controls ?? [];
    const controlIds = controls.map((control) => control.id);
    expect(new Set(controlIds).size).toBe(controlIds.length);
    expect(controlIds).toEqual(expect.arrayContaining([
      "game-analysis-map",
      "game-analysis-altitude",
      "game-analysis-speed",
      "game-result-open-summary",
      "game-result-replay",
      "game-result-retry",
      "game-result-setup",
      "game-result-title"
    ]));
    const chartControl = controls.find((control) => control.kind === "chart");
    if (chartControl?.kind !== "chart") throw new Error("Analysis chart is missing");
    expect(chartControl.rect).toMatchObject({ x: 0.04, y: 0.105, width: 0.92, height: 0.49 });
    for (const id of ["game-analysis-map", "game-analysis-altitude", "game-analysis-speed"] as const) {
      const selector = controls.find((control) => control.id === id);
      expect(selector?.rect.y).toBe(0.025);
    }
    for (const id of ["game-result-open-summary", "game-result-replay", "game-result-retry", "game-result-setup", "game-result-title"] as const) {
      const action = controls.find((control) => control.id === id);
      if (action?.rect === undefined) throw new Error(`Result action has no layout: ${id}`);
      expect(action.rect.y).toBeGreaterThanOrEqual(0.82);
      expect(action.rect.y).toBeLessThan(1);
      expect(action.rect.y + action.rect.height).toBeLessThanOrEqual(0.98);
    }
    if (cursorValues?.kind !== "status") throw new Error("Analysis cursor status is missing");
    expect(cursorValues.value).toContain("t 1.00 s · N 8.0 m · E 6.0 m · h 8.0 m");
    expect(cursorValues.value).toContain("WN 2.0 · WE -1.0 · WD 0.0 m/s");
    if (chart?.kind !== "chart") throw new Error("Analysis chart is missing");
    if (analysisChart === "map") {
      expect(chart.xAxisLabel).toBe("東 E (m)");
      expect(chart.equalAxisScale).toBe(true);
      expect(chart.series[0]?.points[1]).toEqual({ x: 6, y: 8 });
      expect(chart.xMaximum - chart.xMinimum).toBeCloseTo(chart.yMaximum - chart.yMinimum);
      expect((chart.yMaximum - chart.yMinimum) * 0.75).toBeCloseTo(8);
      for (const series of chart.series.slice(1)) {
        for (const point of series.points) {
          expect(point.x).toBeGreaterThanOrEqual(chart.xMinimum);
          expect(point.x).toBeLessThanOrEqual(chart.xMaximum);
          expect(point.y).toBeGreaterThanOrEqual(chart.yMinimum);
          expect(point.y).toBeLessThanOrEqual(chart.yMaximum);
        }
      }
      expect(chart.vectors[0]?.label).toContain("矢印は空気の移動先");
      expect(chart.series[0]?.segmentColors).toHaveLength(1);
      const wind = chart.vectors[0];
      if (wind?.start === null || wind?.start === undefined || wind.end === null) throw new Error("Wind vector is missing");
      expect(wind.end.x).toBeLessThan(wind.start.x);
      expect(wind.end.y).toBeGreaterThan(wind.start.y);
      const windGrid = Object.freeze({
        altitudeMeters: 10,
        samples: Object.freeze(Array.from({ length: 25 }, (_, index) => Object.freeze({
          northMeters: Math.floor(index / 5) * 10,
          eastMeters: (index % 5) * 10,
          windNorthMetersPerSecond: 1,
          windEastMetersPerSecond: -2,
          windDownMetersPerSecond: 0.5
        })))
      });
      const gridAnalysis = Object.freeze({ ...analysis, windGrid });
      const gridView = createGameViewModel({ ...model, flightAnalysis: gridAnalysis }, null, gridAnalysis);
      const gridChart = gridView.panels[0]?.controls.find((control) => control.kind === "chart");
      if (gridChart?.kind !== "chart") throw new Error("Wind grid chart is missing");
      expect(gridChart.vectors).toHaveLength(26);
      expect(gridChart.vectors[1]?.label).toContain("風断面 h 10.0 m");
      expect(gridChart.vectors[1]?.label).toContain("W_D 0.5…0.5 m/s");
      expect(gridChart.vectors[1]?.end?.x).toBeLessThan(gridChart.vectors[1]?.start?.x ?? 0);
      expect(gridChart.vectors[1]?.end?.y).toBeGreaterThan(gridChart.vectors[1]?.start?.y ?? 0);
      const extendedAnalysis = Object.freeze({
        ...analysis,
        samples: Object.freeze([...analysis.samples, sample(2, 16, 12, 6, 7, 8)]),
        summary: Object.freeze({ ...analysis.summary, sampleCount: 3, durationSeconds: 2,
          terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 2 }) })
      });
      const timeSeries = createGameViewModel({ ...model, flightAnalysis: extendedAnalysis }, null, extendedAnalysis)
        .panels[0]?.controls.find((control) => control.kind === "chart");
      if (timeSeries?.kind !== "chart") throw new Error("Time-coloured trajectory is missing");
      expect(timeSeries.series[0]?.segmentColors).toEqual(["#440154", "#fde725"]);
      expect(timeSeries.timeMarkers).toEqual([{ label: "1 s", color: "#d5e0dc", point: { x: 6, y: 8 } }]);
    } else if (analysisChart === "altitude") {
      expect(chart.yAxisLabel).toBe("Altitude (m)");
      expect(chart.series[0]?.points[1]?.y).toBe(8);
    } else {
      expect(chart.series).toHaveLength(2);
      expect(chart.series[0]?.label).toBe("対気速度");
      expect(chart.series[1]?.label).toBe("対地速度");
    }
  });

  it("shows versioned schematic venue geometry only for known synthetic scenarios", () => {
    const base = createInitialAppModel();
    const samples = [0, 1].map((timeSeconds) => Object.freeze({
      timeSeconds, northMeters: timeSeconds * 8, eastMeters: timeSeconds * 6, altitudeMeters: 10 - timeSeconds * 2,
      airspeedMetersPerSecond: 9, groundspeedMetersPerSecond: 10,
      windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0, windDownMetersPerSecond: 0,
      angleOfAttackRadians: null, sideslipRadians: null, rollRadians: 0, pitchRadians: 0, headingRadians: 0
    }));
    const analysis = Object.freeze({
      samples: Object.freeze(samples),
      initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 1, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 1 })
      })
    });
    const configurationMetadata = Object.freeze({
      presetCode: 0, informationCode: 0, hudProfile: createInitialAppModel().difficulty.hudProfile,
      assistanceCode: 0, weatherCode: 0,
      catalogVersion: 1, scenarioId: 1, scenarioVersion: 1, aircraftModelVersion: 1,
      environmentVersion: 1, controllerProfileVersion: 1, seedLow: 0, seedHigh: 0
    });
    const createMap = (scenarioId: number) => {
      const model = Object.freeze({
        ...base, gameSession: sessionForPhase(7), resultTab: "analysis" as const, analysisChart: "map" as const,
        flightAnalysis: analysis, configurationMetadata: Object.freeze({ ...configurationMetadata, scenarioId })
      });
      return createGameViewModel(model, null, analysis).panels[0]?.controls.find((control) => control.kind === "chart");
    };
    const knownScenarioChart = createMap(1);
    const unknownScenarioChart = createMap(99);
    if (knownScenarioChart?.kind !== "chart" || unknownScenarioChart?.kind !== "chart") {
      throw new Error("Analysis map is missing");
    }
    expect(knownScenarioChart.series.map((series) => series.label)).toContain(
      "発進台"
    );
    expect(knownScenarioChart.series.map((series) => series.label)).not.toContain(
      "彦根 湖岸線（OSM）"
    );
    expect(unknownScenarioChart.series).toHaveLength(1);
    expect(unknownScenarioChart.markers).toHaveLength(2);
  });

  it("changes chart selection through the TEA update function", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(7) });
    const transition = updateApp(model, {
      type: "ui-action", action: { type: "activate", controlId: "game-analysis-speed" }
    });
    expect(transition.model.analysisChart).toBe("speed");
    expect(transition.effects).toEqual([]);
  });

  it("fits map plots to a square pixel viewport in Screen and VR", () => {
    const screen = fitPlotRectToEqualScale(78, 34, 970, 420, true);
    const vr = fitPlotRectToEqualScale(90, 100, 980, 700, true);
    expect(screen.right - screen.left).toBe(screen.bottom - screen.top);
    expect(vr.right - vr.left).toBe(vr.bottom - vr.top);
    expect(fitPlotRectToEqualScale(0, 0, 100, 50, false)).toEqual({ left: 0, top: 0, right: 100, bottom: 50 });
  });

  it("formats stable numeric axis labels", () => {
    expect([formatChartTick(-0), formatChartTick(0.125), formatChartTick(12.34), formatChartTick(120.4)])
      .toEqual(["0", "0.13", "12.3", "120"]);
    expect(() => formatChartTick(Number.NaN)).toThrow();
  });

  it("selects a readable metric scale bar distance", () => {
    expect(chartScaleBarDistance(100)).toBe(20);
    expect(chartScaleBarDistance(2)).toBe(0.5);
    expect(() => chartScaleBarDistance(0)).toThrow();
  });

  it("queries and rejects stale Analysis cursor responses through TEA", () => {
    const analysis = Object.freeze({
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const loaded = updateApp(Object.freeze({
      ...createInitialAppModel(), gameSession: sessionForPhase(7), pendingAnalysisRequestId: 4, nextAnalysisCursorRequestId: 8
    }), { type: "flight-analysis-loaded", requestId: 4, data: analysis });
    expect(loaded.effects).toEqual([{ type: "load-flight-analysis-cursor", requestId: 8, timeSeconds: 0 }]);
    const moved = updateApp(loaded.model, {
      type: "ui-action", action: { type: "set-range", controlId: "game-analysis-cursor", value: 5 }
    });
    expect(moved.model.analysisCursorTimeSeconds).toBe(2);
    expect(moved.effects).toEqual([{ type: "load-flight-analysis-cursor", requestId: 9, timeSeconds: 2 }]);
    const stale = updateApp(moved.model, {
      type: "flight-analysis-cursor-loaded", requestId: 8,
      sample: Object.freeze({
        timeSeconds: 0, northMeters: 0, eastMeters: 0, altitudeMeters: 0,
        airspeedMetersPerSecond: 0, groundspeedMetersPerSecond: 0,
        windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0, windDownMetersPerSecond: 0,
        angleOfAttackRadians: null, sideslipRadians: null, rollRadians: 0, pitchRadians: 0, headingRadians: 0
      })
    });
    expect(stale.model.analysisCursorSample).toBeNull();
    expect(stale.model.pendingAnalysisCursorRequestId).toBe(9);
  });

  it("draws the active analysis chart into the VR panel canvas", () => {
    const model = Object.freeze({ ...createInitialAppModel(), gameSession: sessionForPhase(7), resultTab: "analysis" as const, analysisChart: "altitude" as const });
    const analysis = Object.freeze({
      samples: Object.freeze([{
        timeSeconds: 0, northMeters: 0, eastMeters: 0, altitudeMeters: 10,
        airspeedMetersPerSecond: 9, groundspeedMetersPerSecond: 10,
        windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0, windDownMetersPerSecond: 0,
        angleOfAttackRadians: null, sideslipRadians: null, rollRadians: 0, pitchRadians: 0, headingRadians: 0
      }, {
        timeSeconds: 1, northMeters: 8, eastMeters: 6, altitudeMeters: 8,
        airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
        windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0, windDownMetersPerSecond: 0,
        angleOfAttackRadians: null, sideslipRadians: null, rollRadians: 0, pitchRadians: 0, headingRadians: 0
      }]),
      initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 1, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 1 })
      })
    });
    const view = createGameViewModel(model, null, analysis);
    const commands: string[] = [];
    const context: PanelDrawingContext = {
      clearRect: () => commands.push("clear"),
      fillRect: () => commands.push("fill"),
      fillText: (value) => commands.push(value),
      strokeRect: () => commands.push("rect"),
      beginPath: () => commands.push("path"),
      closePath: () => undefined,
      rect: () => undefined,
      clip: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      fill: () => undefined,
      moveTo: () => undefined,
      lineTo: () => undefined,
      stroke: () => commands.push("stroke"),
      setFillStyle: () => undefined,
      setStrokeStyle: () => undefined,
      setFont: () => undefined,
      setTextBaseline: () => undefined,
      setLineWidth: () => undefined,
      setGlobalAlpha: () => undefined
    };
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Result panel is missing");
    drawVrPanel(context, panel, 1024, 768);
    expect(commands).toContain("Altitude vs time");
    expect(commands).toContain("Altitude (m)");
    expect(commands).toContain("0");
    expect(commands).toContain("stroke");
    const mapPanel = createGameViewModel({ ...model, analysisChart: "map" }, null, analysis).panels[0];
    if (mapPanel === undefined) throw new Error("Map panel is missing");
    drawVrPanel(context, mapPanel, 1024, 768);
    expect(commands).toContain("N");
    expect(commands.some((value) => /^\d+(\.\d+)? m$/.test(value))).toBe(true);
  });
});

function sessionForPhase(phaseCode: number, canResume = false) {
  const snapshot = phaseCode === 5 || phaseCode === 6 ? flightSnapshot : null;
  const session = gameSessionState(phaseCode, 0, snapshot, canResume);
  if (session === null) throw new Error(`Invalid fixture game phase ${String(phaseCode)}`);
  return session;
}
