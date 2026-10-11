import { describe, expect, it } from "vitest";
import { DataTexture, Group, Mesh, MeshLambertMaterial } from "three";
import { createBirdmanAirframe, wingDeflectionMeters, wingDihedralMeters } from "../../web/src/render/engines/three/birdman-airframe.js";

describe("synthetic Birdman airframe", () => {
  it("bends both wing tips upward under the visual reference load", () => {
    const halfSpan = 20.863 / 2;
    const tip = wingDeflectionMeters(halfSpan, 9.5);
    expect(tip).toBeGreaterThan(0.3);
    expect(tip).toBeLessThan(0.8);
    expect(wingDeflectionMeters(0, 9.5)).toBe(0);
    expect(wingDeflectionMeters(6, 9.5)).toBeLessThan(tip);
    expect(wingDeflectionMeters(halfSpan, 13)).toBeGreaterThan(tip);
  });

  it("has continuous dihedral breaks near 60% and 80% span", () => {
    const span = 20.863 / 2;
    const rise = (from: number, to: number): number =>
      wingDihedralMeters(to * span) - wingDihedralMeters(from * span);
    expect(rise(0.60, 0.61)).toBeGreaterThan(rise(0.59, 0.60) * 4);
    expect(rise(0.80, 0.81)).toBeGreaterThan(rise(0.79, 0.80) * 2);
    expect(wingDihedralMeters(0.6 * span)).toBeCloseTo(0.008 * 0.6 * span);
    expect(wingDihedralMeters(0.8 * span)).toBeCloseTo(
      wingDihedralMeters(0.6 * span) + 0.045 * 0.2 * span
    );
  });

  it("keeps finite wing geometry, visual bending and cockpit material boundaries", () => {
    const airframe = createBirdmanAirframe();
    try {
      const left = airframe.root.getObjectByName("left-wing") as Mesh;
      const right = airframe.root.getObjectByName("right-wing") as Mesh;
      const foam = airframe.root.getObjectByName("left-styrofoam-rib") as Mesh;
      const cockpit = airframe.root.getObjectByName("open-cockpit-frame");
      const canopy = airframe.root.getObjectByName("transparent-canopy") as Mesh;
      const fairing = airframe.root.getObjectByName("rear-cockpit-fairing") as Mesh;
      expect(left).toBeInstanceOf(Mesh);
      expect(right).toBeInstanceOf(Mesh);
      expect(cockpit).toBeInstanceOf(Group);
      expect(cockpit?.children.length).toBeGreaterThan(8);
      expect((canopy.material as MeshLambertMaterial).transparent).toBe(true);
      expect((fairing.material as MeshLambertMaterial).transparent).toBe(false);
      const [wingCover, wingUnderside] = left.material as MeshLambertMaterial[];
      expect(wingCover?.map).toBeInstanceOf(DataTexture);
      expect(wingUnderside?.map).toBeNull();
      expect(wingUnderside?.color.getHex()).toBe(0xf2f3ed);
      expect(left.geometry.groups.map((group) => group.materialIndex)).toEqual([0, 1, 1]);
      const leftPosition = left.geometry.getAttribute("position");
      const rightPosition = right.geometry.getAttribute("position");
      const tip = leftPosition.count - 1;
      const initialY = leftPosition.getY(tip);
      const initialFoamY = foam.position.y;
      airframe.setVisualState(13, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } },
        { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: 3.6 } });
      expect(leftPosition.getY(tip)).toBeGreaterThan(initialY);
      expect(foam.position.y).toBeGreaterThan(initialFoamY);
      expect(airframe.root.getObjectByName("horizontal-tail-incidence")?.rotation.x).toBeCloseTo(0.12);
      expect(airframe.root.getObjectByName("vertical-tail-incidence")?.rotation.y).toBeCloseTo(0.08);
      expect(leftPosition.count).toBe(rightPosition.count);
      for (const mesh of [left, right]) for (const name of ["position", "normal"]) {
        const attribute = mesh.geometry.getAttribute(name);
        for (let index = 0; index < attribute.count; index++) {
          expect(Number.isFinite(attribute.getX(index))).toBe(true);
          expect(Number.isFinite(attribute.getY(index))).toBe(true);
          expect(Number.isFinite(attribute.getZ(index))).toBe(true);
        }
      }
    } finally { airframe.dispose(); }
  });
});
