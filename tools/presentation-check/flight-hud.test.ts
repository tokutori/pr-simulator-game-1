import { describe, expect, it } from "vitest";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
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

    expect(root.children).toHaveLength(8);
    hud.setVisible(true);
    const firstValues = new Array<number>(33).fill(0);
    firstValues[7] = 1;
    firstValues[19] = -1;
    const firstSnapshot = parseFlightSnapshot(firstValues);
    hud.render(firstSnapshot);
    expect(root.hidden).toBe(false);

    hud.setVisible(false);
    const secondValues = new Array<number>(33).fill(0);
    secondValues[0] = 1;
    secondValues[7] = 1;
    secondValues[16] = 2;
    secondValues[19] = -1;
    secondValues[20] = 12;
    secondValues[21] = 10;
    secondValues[22] = 9;
    secondValues[23] = 2;
    secondValues[24] = -1;
    secondValues[25] = 0.5;
    secondValues[26] = 0.1;
    secondValues[28] = 0.2;
    secondValues[29] = 0.05;
    secondValues[30] = 0.3;
    secondValues[31] = 1;
    const secondSnapshot = parseFlightSnapshot(secondValues);
    hud.render(secondSnapshot);

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
});
