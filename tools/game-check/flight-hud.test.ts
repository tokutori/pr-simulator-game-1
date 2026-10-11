import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { currentSharedHudFixture, withSharedGameUiSession } from "./shared-game-ui-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";

describe("shared flight HUD instruments", () => {
  it("projects the authoritative Rust HUD without independently formatting telemetry or policy", () => {
    withSharedGameUiSession(5, {}, (session, platform) => {
      const view = session.readSharedUi(platform);
      if (view.hud === null) throw new Error("Missing shared HUD");
      const model = projectSharedFlightHudModel(view.hud, view.theme);
      expect(model.attitude).toEqual({
        rollDegrees: view.hud.attitude?.roll_degrees, pitchDegrees: view.hud.attitude?.pitch_degrees
      });
      expect(model.readouts).toBe(view.hud.readouts);
      expect(model.headingDegrees).toBe(view.hud.heading_degrees);
      expect(model.windDirectionDegrees).toBe(view.hud.wind_direction_degrees);
      expect(model.pilotPosition).toBe(view.hud.pilot_position);
      expect(model.angleOfAttackDegrees).toBe(view.hud.angle_of_attack_degrees);
      expect(model.layout).toBe(view.hud.layout);
      expect(model.theme).toBe(view.theme);
    });
  });

  it("preserves absent flow directions and angles at the display boundary", async () => {
    const base = currentSharedHudFixture({ weatherCode: 0 });
    expect(base.windDirectionDegrees).toBeNull();
    const model = Object.freeze({
      ...base, angleOfAttackDegrees: null, angleOfAttack: "—", windDirectionDegrees: null,
      geometry: Object.freeze({ ...base.geometry, angle_of_attack_ratio: null })
    });
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const adapter = new FlightHudAdapter(root);
    adapter.render(model);
    expect(root.querySelector(".flight-hud-instrument-angle output")?.textContent).toBe("—");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("visibility")).toBe("hidden");
    const windNeedle = root.querySelector('[data-instrument="wind"] g');
    expect(windNeedle?.getAttribute("visibility")).toBe("hidden");
    expect(windNeedle?.getAttribute("transform")).toBeNull();
    await window.happyDOM.abort();
  });

  it("receives Custom cue visibility from the Rust owner", () => {
    const model = currentSharedHudFixture({ informationCode: 4, hudProfile: {
      telemetry: false, attitude: false, wind: true, flightPath: true, angleOfAttack: true, warnings: true
    } });
    expect(model.attitude).toBeNull();
    expect(model.heading).toBeNull();
    expect(model.telemetry).toBe("");
    expect(model.wind).not.toBeNull();
    expect(model.angleOfAttack).not.toBeNull();
    expect(model.flightPathAngleDegrees).not.toBeNull();
  });

  it("updates compass tape and axis gauges from each immutable shared model", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const adapter = new FlightHudAdapter(root);
    const base = currentSharedHudFixture();
    adapter.render(Object.freeze({
      ...base, pilotPosition: "-0.20 m", windDirectionDegrees: 270, headingDegrees: 90,
      heading: "90°", angleOfAttackDegrees: 0, angleOfAttack: "0.0°",
      geometry: Object.freeze({
        ...base.geometry, angle_of_attack_ratio: 1 / 3,
        heading_ticks: Object.freeze([
          Object.freeze({ label: "60", major: true, position_ratio: 0.2 }),
          Object.freeze({ label: "E", major: true, position_ratio: 0.5 }),
          Object.freeze({ label: "120", major: true, position_ratio: 0.8 })
        ])
      })
    }));
    expect([...root.querySelectorAll('[data-instrument="heading"] text')].map((label) => label.textContent)).toEqual(["60", "E", "120"]);
    expect([...root.querySelectorAll('[data-instrument="heading"] text')].map((label) => label.getAttribute("x"))).toEqual(["44", "110", "176"]);
    expect(root.querySelector('[data-instrument="wind"] g')?.getAttribute("transform")).toBe("rotate(270 36 36)");
    expect(root.querySelector(".flight-hud-instrument-pilot-position output")?.textContent).toBe("-0.20 m");
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("70,5 64,1 76,1");
    expect(root.querySelector(".flight-hud-adi")?.getAttribute("aria-label")).toBe("姿勢指示器");
    adapter.render(Object.freeze({
      ...base, headingDegrees: 180, heading: "180°", angleOfAttackDegrees: 15, angleOfAttack: "15.0°",
      geometry: Object.freeze({
        ...base.geometry, angle_of_attack_ratio: 5 / 6,
        heading_ticks: Object.freeze([
          Object.freeze({ label: "150", major: true, position_ratio: 0.2 }),
          Object.freeze({ label: "S", major: true, position_ratio: 0.5 }),
          Object.freeze({ label: "210", major: true, position_ratio: 0.8 })
        ])
      })
    }));
    expect(root.querySelector(".flight-hud-instrument-heading output")?.textContent).toBe("180°");
    expect([...root.querySelectorAll('[data-instrument="heading"] text')].map((label) => label.textContent)).toEqual(["150", "S", "210"]);
    expect(root.querySelector('[data-instrument="angle-of-attack"] polygon')?.getAttribute("points")).toBe("160,5 154,1 166,1");
    await window.happyDOM.abort();
  });

});
