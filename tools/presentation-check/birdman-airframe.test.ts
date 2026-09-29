import { describe, expect, it } from "vitest";
import { Mesh, MeshLambertMaterial } from "three";
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
    const film = airframe.root.getObjectByName("left-trailing-film") as Mesh;
    const rib = airframe.root.getObjectByName("left-trailing-rib") as Mesh;
    const cockpit = airframe.root.getObjectByName("enclosed-cockpit") as Mesh;
    const canopy = airframe.root.getObjectByName("opaque-canopy") as Mesh;
    const elevatorFilm = airframe.root.getObjectByName("left-elevator-film") as Mesh;
    const elevator = airframe.root.getObjectByName("elevator");
    const rudder = airframe.root.getObjectByName("rudder");
    expect(left).toBeInstanceOf(Mesh);
    expect(right).toBeInstanceOf(Mesh);
    expect(film).toBeInstanceOf(Mesh);
    expect(rib).toBeInstanceOf(Mesh);
    expect(cockpit).toBeInstanceOf(Mesh);
    expect(canopy).toBeInstanceOf(Mesh);
    expect(elevatorFilm).toBeInstanceOf(Mesh);
    expect((film.material as MeshLambertMaterial).transparent).toBe(true);
    expect((film.material as MeshLambertMaterial).opacity).toBeLessThan(0.3);
    expect((canopy.material as MeshLambertMaterial).transparent).toBe(false);
    expect((elevatorFilm.material as MeshLambertMaterial).transparent).toBe(true);
    expect(airframe.root.getObjectByName("elevator-rib")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-front")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-trailing-film")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-rib")).toBeInstanceOf(Mesh);
    expect(airframe.root.getObjectByName("vertical-stabilizer")).toBeUndefined();
    expect(airframe.root.getObjectByName("pilot-head")).toBeUndefined();
    expect(elevator).toBeDefined();
    expect(rudder).toBeDefined();
    const leftPosition = left.geometry.getAttribute("position");
    const rightPosition = right.geometry.getAttribute("position");
    const tipVertex = leftPosition.count - 1;
    const initialY = leftPosition.getY(tipVertex);
    const initialRibY = rib.position.y;
    airframe.setVisualState(13, 0.12, -0.08);
    expect(leftPosition.getY(tipVertex)).toBeGreaterThan(initialY);
    expect(rib.position.y).toBeGreaterThan(initialRibY);
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
