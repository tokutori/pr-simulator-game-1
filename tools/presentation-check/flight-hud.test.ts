import { describe, expect, it } from "vitest";
import { currentFlightDisplayFixture } from "../game-check/current-session-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";

class TestElement {
  readonly attributes = new Map<string, string>();
  readonly children: TestElement[] = [];
  readonly classes = new Set<string>();
  className = "";
  hidden = false;
  textContent: string | null = "";
  id = "";
  readonly classList = {
    add: (value: string): void => { this.classes.add(value); },
    remove: (value: string): void => { this.classes.delete(value); }
  };

  constructor(readonly ownerDocument: TestDocument) {}

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }

  append(...nodes: TestElement[]): void {
    this.children.push(...nodes);
  }

  replaceChildren(...nodes: TestElement[]): void {
    this.children.splice(0, this.children.length, ...nodes);
  }
}

class TestDocument {
  createElement(): TestElement {
    return new TestElement(this);
  }

  createElementNS(): TestElement {
    return new TestElement(this);
  }
}

describe("Flight HUD DOM lifecycle", () => {
  it("preserves HUD nodes while hidden and updates them before showing again", () => {
    const root = new TestElement(new TestDocument());
    const hud = new FlightHudAdapter(root as unknown as HTMLElement);
    const nodes = [...root.children];

    expect(root.children).toHaveLength(10);
    hud.setVisible(true);
    const firstValuesSnapshot = {
    ...currentFlightDisplayFixture(5),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 0, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 0, airspeedMetersPerSecond: 0, groundspeedMetersPerSecond: 0,
      windVelocityNedMetersPerSecond: { north: 0, east: 0, down: 0 }, angleOfAttackRadians: { kind: "unavailable" as const, reason: "undefined_flow_angle" as const }, sideslipAngleRadians: { kind: "unavailable" as const, reason: "undefined_flow_angle" as const },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
    const firstSnapshot = firstValuesSnapshot;
    hud.renderDisplaySnapshot(firstSnapshot);
    expect(root.hidden).toBe(false);

    hud.setVisible(false);
    const secondValuesSnapshot = {
    ...currentFlightDisplayFixture(7),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 0, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 1, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 10, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 2, east: -1, down: 0.5 }, angleOfAttackRadians: { kind: "available" as const, value: 0.1 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0.2, pitchRadians: 0.05, headingRadians: 0.3 } },
    finalization: { reason: "time_limit" as const, disposition: "complete" as const, terminalTick: 1, terminalFraction: 0, scoreMeters: [0, 0, 0] as const, failure: null }
  };
    const secondSnapshot = secondValuesSnapshot;
    hud.renderDisplaySnapshot(secondSnapshot);

    expect(root.hidden).toBe(true);
    expect(root.attributes.get("aria-hidden")).toBe("true");
    expect(root.children).toEqual(nodes);
    hud.setVisible(true);
    expect(root.hidden).toBe(false);
    expect(root.attributes.get("aria-hidden")).toBe("false");
    expect(root.children).toEqual(nodes);
    expect(nodes[1]?.textContent).toBe("時間制限");
    const instruments = nodes.find((node) => node.className === "flight-hud-instruments")?.children;
    expect(instruments?.[0]?.children[1]?.textContent).toBe("17°");
    expect(instruments?.[1]?.children[1]?.textContent).toBe("+0.00 m");
    expect(instruments?.[2]?.children[1]?.textContent).toBe("N 2.0 · E -1.0 · D 0.5 m/s");
    expect(instruments?.[3]?.children[1]?.textContent).toBe("5.7°");
  });

  it("shows Copernicus source and processing notices by default", () => {
    const root = new TestElement(new TestDocument());
    new FlightHudAdapter(root as unknown as HTMLElement);
    const attribution = root.children.find((node) => node.className === "flight-hud-attributions");
    const sourceNotice = attribution?.children.find((node) => node.className === "flight-hud-attribution-link" && node.textContent?.startsWith("© DLR e.V."));
    const licenseNotice = attribution?.children.find((node) => node.className === "flight-hud-attribution-notice");

    expect(sourceNotice?.textContent).toContain("provided under COPERNICUS by the European Union and ESA; all rights reserved.");
    expect(licenseNotice?.attributes.get("open")).toBeUndefined();
    expect(licenseNotice?.children[1]?.textContent).toBe("produced using Copernicus WorldDEM-30.");
    expect(licenseNotice?.children[2]?.textContent).toContain("do not incur any liability");
  });
});
