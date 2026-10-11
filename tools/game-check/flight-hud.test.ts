import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { currentFlightDisplayFixture } from "./current-session-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { createFlightDisplayHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { drawVrFlightInstruments } from "../../web/src/presentation/vr-panel-canvas.js";
import type { PanelDrawingContext } from "../../web/src/presentation/vr-panel-canvas.js";

describe("flight HUD instruments", () => {
  it("derives axis readouts and gauge inputs from Rust telemetry", () => {
    const model = createFlightDisplayHudModel(flightSnapshot({
      pilotPosition: 0.2,
      windEast: 2,
      pitchDegrees: 5,
      rollDegrees: -10,
      headingDegrees: 450,
      angleOfAttackDegrees: 6
    }), 0);

    expect(model.attitude).toEqual({ rollDegrees: -10, pitchDegrees: 5 });
    expect(model.headingDegrees).toBe(90);
    expect(model.pilotPosition).toBe("+0.20 m");
    expect(model.windDirectionDegrees).toBe(90);
    expect(model.angleOfAttackDegrees).toBe(6);
  });

  it("marks calm wind and undefined angle of attack without inventing directions", () => {
    const snapshot = flightSnapshot({ airspeed: 0, pilotPosition: 1.2 });
    const model = createFlightDisplayHudModel(snapshot, 0);

    expect(model.windDirectionDegrees).toBeNull();
    expect(model.angleOfAttackDegrees).toBeNull();
    expect(model.pilotPosition).toBe("+1.20 m");
  });

  it("applies Custom cue visibility and derives flight-path and warning cues", () => {
    const snapshot = flightSnapshot({ velocityNorth: 10, velocityDown: 1, terminal: 3 });
    const profile = {
      telemetry: false,
      attitude: false,
      wind: true,
      flightPath: true,
      angleOfAttack: true,
      warnings: true
    };
    const model = createFlightDisplayHudModel(snapshot, 4, profile);

    expect(model.attitude).toBeNull();
    expect(model.heading).toBeNull();
    expect(model.telemetry).toBe("");
    expect(model.wind).not.toBeNull();
    expect(model.angleOfAttack).not.toBeNull();
    expect(model.flightPathAngleDegrees).toBeCloseTo(-Math.atan2(1, 10) * 180 / Math.PI);
    expect(model.warning).toBe("AERODYNAMIC ENVELOPE");
  });

  it("updates compass tape and axis gauges from each immutable snapshot", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const adapter = new FlightHudAdapter(root);
    adapter.setInformationProfile(0, {
      telemetry: true,
      attitude: true,
      wind: true,
      flightPath: true,
      angleOfAttack: true,
      warnings: true
    });
    adapter.render(flightSnapshot({
      pilotPosition: -0.2,
      windNorth: 0,
      windEast: -2,
      headingDegrees: 90,
      angleOfAttackDegrees: 0
    }));

    expect(root.querySelector('[data-instrument="heading"]')?.querySelectorAll("text")).toHaveLength(5);
    expect(root.querySelector('[data-instrument="wind"] g')?.getAttribute("transform")).toBe("rotate(270 36 36)");
    expect(root.querySelector(".flight-hud-instrument-pilot-position output")?.textContent).toBe("-0.20 m");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("70,5 64,1 76,1");
    expect(root.querySelector(".flight-hud-adi")?.getAttribute("aria-label")).toBe("姿勢指示器");

    adapter.render(flightSnapshot({ headingDegrees: 180, angleOfAttackDegrees: 15 }));
    expect(root.querySelector(".flight-hud-instrument-heading output")?.textContent).toBe("180°");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("160,5 154,1 166,1");
    await window.happyDOM.abort();
  });

  it("renders the shared instrument model into the VR panel canvas", () => {
    const drawnText: string[] = [];
    let lineCount = 0;
    const context: PanelDrawingContext = {
      clearRect() {}, fillRect() {}, strokeRect() {}, moveTo() {},
      lineTo(x, y) {
        expect(Number.isFinite(x)).toBe(true);
        expect(Number.isFinite(y)).toBe(true);
        lineCount += 1;
      },
      fillText(text) { drawnText.push(text); },
      beginPath() {}, closePath() {}, rect() {}, clip() {}, save() {}, restore() {}, fill() {},
      stroke() {}, setFillStyle() {}, setStrokeStyle() {},
      setFont() {}, setTextBaseline() {}, setLineWidth() {}, setGlobalAlpha() {}
    };
    const model = createFlightDisplayHudModel(flightSnapshot({
      headingDegrees: 90,
      pitchDegrees: 5,
      rollDegrees: -10,
      pilotPosition: 0.2,
      windEast: 2,
      angleOfAttackDegrees: 6
    }), 0);

    drawVrFlightInstruments(context, model);

    expect(drawnText).toContain("ADI · PITCH / ROLL");
    expect(drawnText).toContain("HDG");
    expect(drawnText).toContain("PILOT POSITION");
    expect(drawnText).toContain("+0.20 m");
    expect(drawnText).toContain("PILOT TARGET 0.20 m [u=0.50]");
    expect(drawnText).toContain("WIND VECTOR");
    expect(drawnText).toContain("ANGLE OF ATTACK");
    expect(drawnText).toContain("90°");
    expect(lineCount).toBeGreaterThan(20);

    drawnText.length = 0;
    drawVrFlightInstruments(context, createFlightDisplayHudModel(flightSnapshot(), 2));
    expect(drawnText).toContain("FLIGHT DATA");
    expect(drawnText).not.toContain("HDG");
    expect(drawnText).not.toContain("WIND VECTOR");
    expect(drawnText).not.toContain("ANGLE OF ATTACK");
  });
});

function flightSnapshot(overrides: {
  readonly pilotPosition?: number;
  readonly airspeed?: number;
  readonly windNorth?: number;
  readonly windEast?: number;
  readonly windDown?: number;
  readonly pitchDegrees?: number;
  readonly rollDegrees?: number;
  readonly headingDegrees?: number;
  readonly angleOfAttackDegrees?: number | null;
  readonly velocityNorth?: number;
  readonly velocityDown?: number;
  readonly terminal?: number;
} = {}) {
  const base = currentFlightDisplayFixture();
  const angle = overrides.angleOfAttackDegrees === null || overrides.airspeed === 0
    ? { kind: "unavailable" as const, reason: "undefined_flow_angle" as const }
    : { kind: "available" as const, value: (overrides.angleOfAttackDegrees ?? 0) * Math.PI / 180 };
  const common = {
    ...base,
    pilotPositionMeters: overrides.pilotPosition ?? 0,
    pilotPositionTargetMeters: { kind: "available" as const, value: overrides.pilotPosition ?? 0 },
    pilotPositionTargetNormalized: { kind: "available" as const, value: (overrides.pilotPosition ?? 0) / 0.4 },
    velocityNed: { north: overrides.velocityNorth ?? 0, east: 0, down: overrides.velocityDown ?? 0 },
    telemetry: { kind: "available" as const, value: {
      ...base.telemetry.value,
      airspeedMetersPerSecond: overrides.airspeed ?? 10,
      windVelocityNedMetersPerSecond: { north: overrides.windNorth ?? 0, east: overrides.windEast ?? 0, down: overrides.windDown ?? 0 },
      angleOfAttackRadians: angle,
      rollRadians: (overrides.rollDegrees ?? 0) * Math.PI / 180,
      pitchRadians: (overrides.pitchDegrees ?? 0) * Math.PI / 180,
      headingRadians: (overrides.headingDegrees ?? 0) * Math.PI / 180
    } }
  };
  const snapshot: FlightDisplaySnapshot = overrides.terminal === 3 ? {
    ...common, kind: "tail_result", progressMeters: { kind: "unavailable", reason: "terminal_progress_unavailable" },
    finalization: { reason: "out_of_valid_envelope", disposition: "failed", terminalTick: base.stamp.tick,
      terminalFraction: 0, scoreMeters: null, failure: null }
  } : common;
  return snapshot;
}
