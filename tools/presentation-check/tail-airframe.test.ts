import { describe, expect, it } from "vitest";
import { Group, Mesh, Vector3 } from "three";
import type { Object3D } from "three";
import { createBirdmanAirframe, NO_AIRFRAME_CONTROLS } from "../../web/src/render/engines/three/birdman-airframe.js";
import type { BirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";
import type { TailPresentationGeometryAvailability } from "../../web/src/render/contracts/flight-controls.js";

const PLAYABLE_GEOMETRY = Object.freeze({ kind: "available",
  value: Object.freeze({ kind: "bpg041_playable_version_two", horizontalTailArmMeters: 3.6 }) }) satisfies TailPresentationGeometryAvailability;

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
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0, verticalTailRadians: 0 } }, PLAYABLE_GEOMETRY);
      expect(horizontal.parent).toBe(airframe.root);
      expect(vertical.parent).toBe(airframe.root);
      expect(horizontal.position.toArray()).toEqual([0, -0.1, 3.6]);
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
      } }, PLAYABLE_GEOMETRY);
      airframe.root.updateMatrixWorld(true);
      const horizontalForward = new Vector3(0, 0, -1).transformDirection(horizontal.matrixWorld);
      const verticalForward = new Vector3(0, 0, -1).transformDirection(vertical.matrixWorld);
      expect(horizontalForward.x).toBeCloseTo(0);
      expect(horizontalForward.y).toBeCloseTo(Math.sin(incidence));
      expect(horizontalForward.z).toBeCloseTo(-Math.cos(incidence));
      expect(verticalForward.x).toBeCloseTo(Math.sin(incidence));
      expect(verticalForward.y).toBeCloseTo(0);
      expect(verticalForward.z).toBeCloseTo(-Math.cos(incidence));
      expect(horizontal.getWorldPosition(new Vector3()).toArray()).toEqual([0, -0.1, 3.6]);
      expect(vertical.getWorldPosition(new Vector3()).toArray()).toEqual([0, 0.1, 1.8]);
      expect(horizontal.visible && vertical.visible).toBe(true);
    } finally {
      airframe.dispose();
    }
  });

  it("resets physical surface visibility and incidence across current and absent controls", () => {
    const airframe = createBirdmanAirframe();
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.1, verticalTailRadians: -0.2 } }, PLAYABLE_GEOMETRY);
      expect(horizontal.visible && vertical.visible).toBe(true);
      expect(horizontal.rotation.x).toBeCloseTo(0.1);
      expect(vertical.rotation.y).toBeCloseTo(0.2);
      airframe.setVisualState(null, NO_AIRFRAME_CONTROLS);
      expect(horizontal.visible || vertical.visible).toBe(false);
      expect(horizontal.rotation.x).toBe(0);
      expect(vertical.rotation.y).toBe(0);
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: -0.1, verticalTailRadians: 0.2 } }, PLAYABLE_GEOMETRY);
      expect(horizontal.visible && vertical.visible).toBe(true);
      expect(horizontal.rotation.x).toBeCloseTo(-0.1);
      expect(vertical.rotation.y).toBeCloseTo(-0.2);
    } finally { airframe.dispose(); }
  });

  it("retains current geometry and controls without rebuilding the surface meshes", () => {
    const airframe = createBirdmanAirframe();
    const controls = Object.freeze({ layout: "tail_incidence" as const, physicalIncidence: Object.freeze({
      horizontalTailRadians: 0.1, verticalTailRadians: -0.2
    }) });
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      const horizontalMesh = surface(horizontal).geometry;
      const verticalMesh = surface(vertical).geometry;
      for (let iteration = 0; iteration < 3; iteration++) {
        airframe.setVisualState(null, controls, PLAYABLE_GEOMETRY);
        airframe.root.updateMatrixWorld(true);
        expect(horizontal.getWorldPosition(new Vector3()).toArray()).toEqual([0, -0.1, 3.6]);
        expect(vertical.getWorldPosition(new Vector3()).toArray()).toEqual([0, 0.1, 1.8]);
        expect(horizontal.rotation.x).toBe(controls.physicalIncidence.horizontalTailRadians);
        expect(vertical.rotation.y).toBe(-controls.physicalIncidence.verticalTailRadians);
        expect(surface(horizontal).geometry).toBe(horizontalMesh);
        expect(surface(vertical).geometry).toBe(verticalMesh);
        airframe.setVisualState(null, NO_AIRFRAME_CONTROLS);
        expect(horizontal.visible || vertical.visible).toBe(false);
      }
      expect(controls.physicalIncidence).toEqual({ horizontalTailRadians: 0.1, verticalTailRadians: -0.2 });
    } finally { airframe.dispose(); }
  });

  it("rejects unavailable or missing tail geometry before changing the visible airframe", () => {
    const airframe = createBirdmanAirframe();
    const controls = { layout: "tail_incidence" as const, physicalIncidence: { horizontalTailRadians: 0.1, verticalTailRadians: -0.2 } };
    try {
      airframe.setVisualState(null, controls, PLAYABLE_GEOMETRY);
      const invalidGeometry: readonly (TailPresentationGeometryAvailability | undefined)[] = [
        undefined, { kind: "unavailable", reason: "unregistered_aircraft_geometry" },
        { kind: "available", value: { kind: "bpg041_version_one", horizontalTailArmMeters: 3.6 } } as unknown as TailPresentationGeometryAvailability,
        { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: Number.NaN } } as unknown as TailPresentationGeometryAvailability
      ];
      for (const geometry of invalidGeometry) {
        expect(() => { airframe.setVisualState(null, { ...controls, physicalIncidence: { horizontalTailRadians: 0.2, verticalTailRadians: 0.2 } }, geometry); }).toThrow("registered aircraft geometry");
        expect(group(airframe, "horizontal-tail-incidence").position.z).toBe(3.6);
        expect(group(airframe, "horizontal-tail-incidence").rotation.x).toBe(0.1);
        expect(group(airframe, "vertical-tail-incidence").rotation.y).toBe(0.2);
      }
    } finally {
      airframe.dispose();
    }
  });

  it("rejects nonfinite physical incidence before changing the current surfaces", () => {
    const airframe = createBirdmanAirframe();
    try {
      expect(() => { airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: Number.NaN, verticalTailRadians: 0
      } }, PLAYABLE_GEOMETRY); }).toThrow("finite");
      expect(group(airframe, "horizontal-tail-incidence").visible).toBe(false);
    } finally {
      airframe.dispose();
    }
  });
});
