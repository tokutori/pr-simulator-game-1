import { describe, expect, it } from "vitest";
import { Group, Mesh, Vector3 } from "three";
import type { Object3D } from "three";
import { createBirdmanAirframe, NO_AIRFRAME_CONTROLS } from "../../web/src/render/engines/three/birdman-airframe.js";
import type { BirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";

function group(airframe: BirdmanAirframe, name: string): Group {
  const node = airframe.root.getObjectByName(name);
  if (!isGroup(node)) throw new Error(`Expected ${name}`);
  return node;
}

function surface(parent: Group): Mesh {
  const node = parent.getObjectByName("physical-tail-surface");
  if (!isMesh(node)) throw new Error("Expected rectangular tail surface");
  return node;
}

function isGroup(node: Object3D | undefined): node is Group {
  return node instanceof Group;
}

function isMesh(node: Object3D | undefined): node is Mesh {
  return node instanceof Mesh;
}

describe("physical two-tail airframe adapter", () => {
  it("uses public rectangular mock spans, chord fractions and independent quarter-chord anchors", () => {
    const airframe = createBirdmanAirframe();
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      expect(horizontal.parent).toBe(airframe.root);
      expect(vertical.parent).toBe(airframe.root);
      expect(horizontal.position.toArray()).toEqual([0, -0.1, 1.8]);
      expect(vertical.position.toArray()).toEqual([0, 0.1, 1.8]);
      const horizontalVertices = surface(horizontal).geometry.getAttribute("position");
      const verticalVertices = surface(vertical).geometry.getAttribute("position");
      expect(horizontalVertices.getX(0)).toBeCloseTo(-1.7);
      expect(horizontalVertices.getX(1)).toBeCloseTo(1.7);
      expect(horizontalVertices.getZ(0)).toBeCloseTo(-(2.5 / 3.4) / 4);
      expect(horizontalVertices.getZ(2)).toBeCloseTo(3 * (2.5 / 3.4) / 4);
      expect(verticalVertices.getY(0) + vertical.position.y).toBeCloseTo(-0.25);
      expect(verticalVertices.getY(1) + vertical.position.y).toBeCloseTo(0.45);
      expect(verticalVertices.getZ(0)).toBeCloseTo(-(0.5 / 0.7) / 4);
      expect(verticalVertices.getZ(2)).toBeCloseTo(3 * (0.5 / 0.7) / 4);
      const span = horizontalVertices.getX(1) - horizontalVertices.getX(0);
      const chord = horizontalVertices.getZ(2) - horizontalVertices.getZ(0);
      expect(span * chord).toBeCloseTo(2.5);
    } finally {
      airframe.dispose();
    }
  });

  it.each([-0.2, 0.1, 0.2])("rotates complete surfaces with FRD-compatible leading-edge vectors at %s rad", (incidence) => {
    const airframe = createBirdmanAirframe();
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: incidence, verticalTailRadians: incidence
      } });
      airframe.root.updateMatrixWorld(true);
      const horizontalForward = new Vector3(0, 0, -1).transformDirection(horizontal.matrixWorld);
      const verticalForward = new Vector3(0, 0, -1).transformDirection(vertical.matrixWorld);
      expect(horizontalForward.x).toBeCloseTo(0);
      expect(horizontalForward.y).toBeCloseTo(Math.sin(incidence));
      expect(horizontalForward.z).toBeCloseTo(-Math.cos(incidence));
      expect(verticalForward.x).toBeCloseTo(Math.sin(incidence));
      expect(verticalForward.y).toBeCloseTo(0);
      expect(verticalForward.z).toBeCloseTo(-Math.cos(incidence));
      expect(horizontal.getWorldPosition(new Vector3()).toArray()).toEqual([0, -0.1, 1.8]);
      expect(vertical.getWorldPosition(new Vector3()).toArray()).toEqual([0, 0.1, 1.8]);
      expect(group(airframe, "legacy-tail-assembly").visible).toBe(false);
      expect(group(airframe, "elevator").rotation.x).toBe(0);
      expect(group(airframe, "rudder").rotation.y).toBe(0);
      expect(horizontal.visible && vertical.visible).toBe(true);
    } finally {
      airframe.dispose();
    }
  });

  it("resets visibility and incidence across tail, legacy, tail and absent controls", () => {
    const airframe = createBirdmanAirframe();
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      const legacy = group(airframe, "legacy-tail-assembly");
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.1, verticalTailRadians: -0.2 } });
      airframe.setVisualState(null, { layout: "legacy_three_axis", rollRadians: 0.1, pitchRadians: 0.12, yawRadians: -0.08 });
      expect(horizontal.visible || vertical.visible).toBe(false);
      expect(legacy.visible).toBe(true);
      expect(horizontal.rotation.x).toBe(0);
      expect(vertical.rotation.y).toBe(0);
      expect(group(airframe, "elevator").rotation.x).toBeCloseTo(-0.12);
      expect(group(airframe, "rudder").rotation.y).toBeCloseTo(-0.08);
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: -0.1, verticalTailRadians: 0.2 } });
      expect(horizontal.rotation.x).toBeCloseTo(-0.1);
      expect(vertical.rotation.y).toBeCloseTo(-0.2);
      expect(group(airframe, "elevator").rotation.x).toBe(0);
      expect(group(airframe, "rudder").rotation.y).toBe(0);
      airframe.setVisualState(null, NO_AIRFRAME_CONTROLS);
      expect(horizontal.visible || vertical.visible).toBe(false);
      expect(legacy.visible).toBe(true);
      expect(horizontal.rotation.x).toBe(0);
      expect(vertical.rotation.y).toBe(0);
      expect(group(airframe, "elevator").rotation.x).toBe(0);
      expect(group(airframe, "rudder").rotation.y).toBe(0);
    } finally {
      airframe.dispose();
    }
  });

  it("rejects nonfinite physical incidence without hiding it behind a legacy control", () => {
    const airframe = createBirdmanAirframe();
    try {
      expect(() => { airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: Number.NaN, verticalTailRadians: 0
      } }); }).toThrow("finite");
      expect(group(airframe, "legacy-tail-assembly").visible).toBe(true);
      expect(group(airframe, "horizontal-tail-incidence").visible).toBe(false);
    } finally {
      airframe.dispose();
    }
  });
});
