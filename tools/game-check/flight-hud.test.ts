import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { currentSharedHudFixture, mapSharedHudNodes, sharedHudNodes, withSharedGameUiSession } from "./shared-game-ui-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";

describe("shared flight HUD instruments", () => {
  it("preserves the authoritative Rust nodes and layout without independent telemetry or policy", () => {
    withSharedGameUiSession(5, {}, (session, platform) => {
      const view = session.readSharedUi(platform);
      if (view.hud === null) throw new Error("Missing shared HUD");
      const model = projectSharedFlightHudModel(view.hud, view.theme);
      expect(model.groups).toBe(view.hud.groups);
      expect(model.layout).toBe(view.hud.layout);
      expect(model.theme).toBe(view.theme);
    });
  });

  it("preserves undefined geometry without inventing instrument values", async () => {
    const base = currentSharedHudFixture({ informationCode: 0, weatherCode: 0 });
    const wind = sharedHudNodes(base).find((node) => node.content.kind === "direction");
    expect(wind?.content).toMatchObject({ rotation_degrees: null });
    const model = mapSharedHudNodes(base, (node) => node.content.kind === "linear_gauge"
      ? { ...node, content: { ...node.content, value: "—", position_ratio: null } } : node);
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    try {
      new FlightHudAdapter(root).render(model);
      expect(root.querySelector('[data-kind="linear_gauge"] output')?.textContent).toBe("—");
      expect(root.querySelector('[data-instrument="linear_gauge"] polygon')).toBeNull();
      expect(root.querySelector('[data-instrument="direction"] g')).toBeNull();
    } finally { await window.happyDOM.abort(); }
  });

  it("receives Custom cue visibility and FlightPathOnly geometry from the Rust owner", () => {
    const model = currentSharedHudFixture({ informationCode: 4, hudProfile: {
      telemetry: false, attitude: false, wind: true, flightPath: true, angleOfAttack: true, warnings: true
    } });
    const contents = sharedHudNodes(model).map((node) => node.content);
    expect(contents.some((content) => content.kind === "heading_tape")).toBe(false);
    expect(contents.find((content) => content.kind === "attitude")).toMatchObject({ geometry: { kind: "flight_path_only" } });
    expect(contents.some((content) => content.kind === "direction")).toBe(true);
    expect(contents.some((content) => content.kind === "linear_gauge")).toBe(true);
    expect(contents.some((content) => content.value.includes("PILOT TARGET"))).toBe(false);
  });
});
