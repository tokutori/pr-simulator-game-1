import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { currentSharedHudFixture } from "../game-check/shared-game-ui-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { gameUiValidatorLicenseUrl } from "../../web/src/app/game-ui-codec.js";

describe("Flight HUD DOM lifecycle", () => {
  it("retains layout writes and heading nodes on numeric frames and applies changed structure", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const model = currentSharedHudFixture();
    const layout = vi.spyOn(hud as unknown as { applyLayout: (value: typeof model) => void }, "applyLayout");
    try {
      hud.render(model);
      expect(layout).toHaveBeenCalledTimes(1);
      const heading = root.querySelector('svg[aria-label="方位目盛"] > g');
      const ticks = Array.from(heading?.childNodes ?? []);
      expect(ticks.length).toBeGreaterThan(0);
      const next = { ...model, layout: { ...model.layout, offset_px: [...model.layout.offset_px] as const },
        geometry: { ...model.geometry, heading_ticks: model.geometry.heading_ticks.map((tick) => ({ ...tick })) },
        readouts: "air 12.3 m/s", telemetry: "time 2.0 s" };
      hud.render(next);
      expect(layout).toHaveBeenCalledTimes(1);
      expect(Array.from(heading?.childNodes ?? [])).toEqual(ticks);
      expect(root.querySelector(".flight-hud-telemetry")?.textContent).toBe(next.telemetry);
      hud.render({ ...next, layout: { ...next.layout, width_px: next.layout.width_px + 10 },
        theme: { ...next.theme, text: "#123456" } });
      expect(layout).toHaveBeenCalledTimes(2);
      expect(root.style.width).toBe(`${String(next.layout.width_px + 10)}px`);
      expect(root.style.color).toBe("#123456");
      hud.render({ ...next, geometry: { ...next.geometry, heading_ticks: [{ position_ratio: 0.5, major: true, label: "X" }] },
        labels: { ...next.labels, heading: "HEADING UPDATED" } });
      expect(heading?.querySelector("text")?.textContent).toBe("X");
      expect(Array.from(heading?.childNodes ?? [])).not.toEqual(ticks);
      expect(root.querySelector(".flight-hud-instruments span")?.textContent).toBe("HEADING UPDATED");
    } finally {
      layout.mockRestore();
      await window.happyDOM.abort();
    }
  });

  it("hides an absent shared observation while retaining HUD nodes and the next model update", async () => {
    const window = new Window();
    const nativeRoot = window.document.createElement("section");
    const root = nativeRoot as unknown as HTMLElement;
    window.document.body.append(nativeRoot);
    const hud = new FlightHudAdapter(root);
    const nodes = Array.from(root.childNodes);
    const initial = currentSharedHudFixture();
    try {
      hud.render(initial);
      hud.setVisible(true);
      expect(root.hidden).toBe(false);
      expect(() => { hud.render(null); }).not.toThrow();
      expect(root.hidden).toBe(true);
      expect(root.getAttribute("aria-hidden")).toBe("true");
      expect(Array.from(root.childNodes)).toEqual(nodes);
      const updated = { ...initial, status: "共有観測を更新した" };
      hud.render(updated);
      expect(root.hidden).toBe(true);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe(updated.status);
      hud.setVisible(true);
      expect(root.hidden).toBe(false);
      expect(nodes.every((node) => node.isConnected)).toBe(true);
    } finally {
      await window.happyDOM.abort();
    }
  });

  it("preserves connected HUD nodes while hidden and applies shared readouts before showing again", async () => {
    const window = new Window();
    const nativeRoot = window.document.createElement("section");
    const root = nativeRoot as unknown as HTMLElement;
    window.document.body.append(nativeRoot);
    const hud = new FlightHudAdapter(root);
    const nodes = Array.from(root.childNodes);
    const initial = currentSharedHudFixture();
    try {
      expect(root.children).toHaveLength(10);
      hud.setVisible(true);
      hud.render(initial);
      expect(root.hidden).toBe(false);
      hud.setVisible(false);
      const updated = { ...initial, status: "時間制限", heading: "17°", pilotPosition: "+0.00 m",
        wind: "N 2.0 · E -1.0 · D 0.5 m/s", angleOfAttack: "5.7°", headingDegrees: 17,
        angleOfAttackDegrees: 5.7, windDirectionDegrees: 90, attitude: { rollDegrees: 11, pitchDegrees: 2 },
        layout: { ...initial.layout, width_px: 280, instrument_columns: 2, controls_visible: false } };
      hud.render(updated);
      expect(root.hidden).toBe(true);
      expect(root.getAttribute("aria-hidden")).toBe("true");
      expect(Array.from(root.childNodes)).toEqual(nodes);
      hud.setVisible(true);
      expect(root.hidden).toBe(false);
      expect(root.getAttribute("aria-hidden")).toBe("false");
      expect(Array.from(root.childNodes)).toEqual(nodes);
      expect(nodes.every((node) => node.isConnected)).toBe(true);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe(updated.status);
      const instruments = Array.from(root.querySelectorAll(".flight-hud-instruments output"));
      expect(instruments.map((node) => node.textContent)).toEqual([updated.heading, updated.pilotPosition, updated.wind, updated.angleOfAttack]);
      expect(root.style.width).toBe("280px");
      expect(root.querySelector<HTMLElement>(".flight-hud-controls")?.hidden).toBe(true);
      expect(Array.from(root.querySelectorAll(".flight-hud-instruments span")).map((node) => node.textContent))
        .toEqual([updated.labels.heading, updated.labels.pilotPosition, updated.labels.wind, updated.labels.angleOfAttack]);
    } finally {
      await window.happyDOM.abort();
    }
  });

  it("shows Copernicus source and processing notices by default", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    try {
      new FlightHudAdapter(root);
      const source = Array.from(root.querySelectorAll(".flight-hud-attribution-link")).find((node) => node.textContent.startsWith("© DLR e.V."));
      const notice = root.querySelector<HTMLDetailsElement>(".flight-hud-attribution-notice");
      expect(source?.textContent).toContain("provided under COPERNICUS by the European Union and ESA; all rights reserved.");
      expect(notice?.open).toBe(true);
      expect(notice?.children[1]?.textContent).toBe("produced using Copernicus WorldDEM-30.");
      expect(notice?.children[2]?.textContent).toContain("do not incur any liability");
      const validatorLicense = Array.from(root.querySelectorAll<HTMLAnchorElement>(".flight-hud-attribution-link"))
        .find((node) => node.textContent === "Game UI validator 利用条件");
      expect(validatorLicense?.href).toBe(gameUiValidatorLicenseUrl);
      expect(validatorLicense?.target).toBe("_blank");
      expect(validatorLicense?.rel).toBe("noopener noreferrer");
    } finally {
      await window.happyDOM.abort();
    }
  });

  it("projects shared instrument geometry without deriving angles or compass labels", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const model = currentSharedHudFixture();
    const geometry = { adi: { kind: "attitude" as const, horizon: { rotation_degrees: -11, pitch_offset_ratio: 0.025 },
      flight_path_y_ratio: 0.6 }, angle_of_attack_ratio: 0.75,
      heading_ticks: [{ position_ratio: 0.5, major: true, label: "N" }, { position_ratio: 0.75, major: false, label: "" }] };
    try {
      hud.render({ ...model, geometry, windDirectionDegrees: 90 });
      expect(root.querySelector(".flight-hud-adi > g")?.getAttribute("transform")).toBe("rotate(-11 120 90) translate(0 94.5)");
      expect(root.querySelector(".flight-hud-adi > circle")?.getAttribute("cy")).toBe("108");
      const heading = root.querySelector('svg[aria-label="方位目盛"]');
      expect(heading?.querySelectorAll("g > path")).toHaveLength(2);
      expect(heading?.querySelector("g > text")?.textContent).toBe("N");
      expect(root.querySelector('svg[aria-label="迎角目盛"] > polygon')?.getAttribute("points")).toBe("145,5 139,1 151,1");
      const needle = root.querySelector('svg[aria-label="風向計。矢印は風の流れる方向を示す"] > g');
      expect(needle?.getAttribute("transform")).toBe("rotate(90 36 36)");
      const retained = Array.from(root.childNodes);
      hud.render({ ...model,
        geometry: { adi: null, angle_of_attack_ratio: null, heading_ticks: [] },
        windDirectionDegrees: null });
      expect(root.querySelector(".flight-hud-adi > circle")?.getAttribute("visibility")).toBe("hidden");
      expect(root.querySelector(".flight-hud-adi")?.classList.contains("is-hidden")).toBe(true);
      expect(root.querySelector('svg[aria-label="迎角目盛"] > polygon')?.getAttribute("visibility")).toBe("hidden");
      expect(needle?.getAttribute("visibility")).toBe("hidden");
      expect(heading?.querySelectorAll("g > path")).toHaveLength(0);
      expect(Array.from(root.childNodes)).toEqual(retained);
    } finally {
      await window.happyDOM.abort();
    }
  });

  it("shows the real shared Custom flight path cue through its visible ADI ancestors", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const model = currentSharedHudFixture({ informationCode: 4,
      hudProfile: { telemetry: false, attitude: false, wind: false, flightPath: true, angleOfAttack: false, warnings: false } });
    try {
      expect(model.attitude).toBeNull();
      const geometry = model.geometry.adi;
      expect(geometry?.kind).toBe("flight_path_only");
      if (geometry?.kind !== "flight_path_only") throw new Error("Expected shared flight-path-only geometry");
      hud.setVisible(true);
      hud.render(model);
      const adi = root.querySelector(".flight-hud-adi");
      const marker = adi?.querySelector("circle");
      expect(root.hidden).toBe(false);
      expect(adi?.classList.contains("is-hidden")).toBe(false);
      expect(adi?.querySelector("g")?.getAttribute("visibility")).toBe("hidden");
      expect(marker?.parentElement).toBe(adi);
      expect(marker?.getAttribute("visibility")).toBe("visible");
      expect(marker?.getAttribute("cy")).toBe(String(geometry.y_ratio * 180));
    } finally {
      await window.happyDOM.abort();
    }
  });
});
