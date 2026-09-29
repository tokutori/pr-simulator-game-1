import { describe, expect, it } from "vitest";
import { Mesh } from "three";
import { createBirdmanAirframe, wingDeflectionMeters } from "../../web/src/render/engines/three/birdman-airframe.js";

describe("synthetic Birdman airframe", () => {
  it("bends both wing tips upward under the visual reference load", () => {
    const tip = wingDeflectionMeters(11.5, 9.5);
    expect(tip).toBeGreaterThan(0.3);
    expect(tip).toBeLessThan(0.8);
    expect(wingDeflectionMeters(0, 9.5)).toBe(0);
    expect(wingDeflectionMeters(6, 9.5)).toBeLessThan(tip);
    expect(wingDeflectionMeters(11.5, 13)).toBeGreaterThan(tip);
  });

  it("has finite airfoil geometry and animates only elevator and rudder", () => {
    const airframe = createBirdmanAirframe();
    const left = airframe.root.getObjectByName("left-wing") as Mesh;
    const right = airframe.root.getObjectByName("right-wing") as Mesh;
    const elevator = airframe.root.getObjectByName("elevator");
    const rudder = airframe.root.getObjectByName("rudder");
    expect(left).toBeInstanceOf(Mesh);
    expect(right).toBeInstanceOf(Mesh);
    expect(elevator).toBeDefined();
    expect(rudder).toBeDefined();
    const leftPosition = left.geometry.getAttribute("position");
    const rightPosition = right.geometry.getAttribute("position");
    const tipVertex = leftPosition.count - 1;
    const initialY = leftPosition.getY(tipVertex);
    airframe.setVisualState(13, 0.12, -0.08);
    expect(leftPosition.getY(tipVertex)).toBeGreaterThan(initialY);
    expect(elevator?.rotation.x).toBeCloseTo(-0.12);
    expect(rudder?.rotation.y).toBeCloseTo(-0.08);
    expect(leftPosition.count).toBe(rightPosition.count);
    for (const geometry of [left.geometry, right.geometry]) {
      for (const attributeName of ["position", "normal"]) {
        const attribute = geometry.getAttribute(attributeName);
        for (let index = 0; index < attribute.count; index++) {
          expect(Number.isFinite(attribute.getX(index))).toBe(true);
          expect(Number.isFinite(attribute.getY(index))).toBe(true);
          expect(Number.isFinite(attribute.getZ(index))).toBe(true);
        }
      }
    }
    airframe.dispose();
  });
});
