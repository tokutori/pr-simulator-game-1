import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { FLIGHT_SNAPSHOT_LENGTH, parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { createFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";

describe("flight HUD instruments", () => {
  it("derives axis readouts and gauge inputs from Rust telemetry", () => {
    const model = createFlightHudModel(flightSnapshot({
      pilotPosition: 0.2,
      windEast: 2,
      pitchDegrees: 5,
      rollDegrees: -10,
      headingDegrees: 450,
      angleOfAttackDegrees: 6
    }), 0);

    expect(model.attitude).toEqual({ rollDegrees: -10, pitchDegrees: 5 });
    expect(model.headingDegrees).toBe(90);
    expect(model.pilotPositionRatio).toBe(0.5);
    expect(model.windDirectionDegrees).toBe(90);
    expect(model.angleOfAttackDegrees).toBe(6);
  });

  it("marks calm wind and undefined angle of attack without inventing directions", () => {
    const snapshot = flightSnapshot({ airspeed: 0, pilotPosition: 1.2 });
    const model = createFlightHudModel(snapshot, 0);

    expect(model.windDirectionDegrees).toBeNull();
    expect(model.angleOfAttackDegrees).toBeNull();
    expect(model.pilotPositionRatio).toBe(1);
  });

  it("updates compass tape and axis gauges from each immutable snapshot", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const adapter = new FlightHudAdapter(root);
    adapter.setInformationCode(0);
    adapter.render(flightSnapshot({
      pilotPosition: -0.2,
      windNorth: 0,
      windEast: -2,
      headingDegrees: 90,
      angleOfAttackDegrees: 0
    }));

    expect(root.querySelector('[data-instrument="heading"]')?.querySelectorAll("text")).toHaveLength(5);
    expect(root.querySelector('[data-instrument="wind"] g')?.getAttribute("transform")).toBe("rotate(270 36 36)");
    expect(root.querySelector('[data-instrument="pilot-position"] polygon')?.getAttribute("points")).toBe("60,5 54,1 66,1");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("70,5 64,1 76,1");
    expect(root.querySelector(".flight-hud-adi")?.getAttribute("aria-label")).toBe("姿勢指示器");

    adapter.render(flightSnapshot({ headingDegrees: 180, angleOfAttackDegrees: 15 }));
    expect(root.querySelector(".flight-hud-instrument-heading output")?.textContent).toBe("180°");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("160,5 154,1 166,1");
    await window.happyDOM.abort();
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
} = {}) {
  const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
  values[7] = 1;
  values[11] = overrides.pilotPosition ?? 0;
  values[19] = -1;
  values[20] = 10;
  values[21] = overrides.airspeed ?? 10;
  values[22] = 10;
  values[23] = overrides.windNorth ?? 0;
  values[24] = overrides.windEast ?? 0;
  values[25] = overrides.windDown ?? 0;
  values[26] = (overrides.angleOfAttackDegrees ?? 0) * Math.PI / 180;
  values[28] = (overrides.rollDegrees ?? 0) * Math.PI / 180;
  values[29] = (overrides.pitchDegrees ?? 0) * Math.PI / 180;
  values[30] = (overrides.headingDegrees ?? 0) * Math.PI / 180;
  values[31] = 1;
  return parseFlightSnapshot(values);
}
