import "./lake-model-fixture.js";
import { describe, expect, it } from "vitest";
import { Box3, Group, Mesh, Vector3 } from "three";
import type { Object3D } from "three";
import { createBirdmanAirframe, NO_AIRFRAME_CONTROLS } from "../../web/src/render/engines/three/birdman-airframe.js";
import type { BirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";
import type { TailPresentationGeometryAvailability } from "../../web/src/render/contracts/flight-controls.js";

const PLAYABLE_GEOMETRY = Object.freeze({ kind: "available",
  value: Object.freeze({ horizontalTailArmMeters: 3.6 }) }) satisfies TailPresentationGeometryAvailability;

function group(airframe: BirdmanAirframe, name: string): Group {
  const node = airframe.root.getObjectByName(name);
  if (!isGroup(node)) throw new Error(`Expected ${name}`);
  return node;
}

function surfaces(parent: Group): readonly [Mesh, Mesh] {
  const cover = parent.getObjectByName("physical-tail-cover");
  const film = parent.getObjectByName("physical-tail-film");
  if (!isMesh(cover) || !isMesh(film)) throw new Error("Expected complete tail cover and film");
  return [cover, film];
}

function outline(parent: Group): Box3 {
  const bounds = new Box3();
  for (const surface of surfaces(parent)) {
    const positions = surface.geometry.getAttribute("position");
    for (let index = 0; index < positions.count; index++) {
      bounds.expandByPoint(new Vector3().fromBufferAttribute(positions, index));
    }
  }
  return bounds;
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
      const horizontalBounds = outline(horizontal);
      const verticalBounds = outline(vertical);
      expect(horizontalBounds.min.x).toBeCloseTo(-1.7);
      expect(horizontalBounds.max.x).toBeCloseTo(1.7);
      expect(horizontalBounds.min.z).toBeCloseTo(-(2.5 / 3.4) / 4);
      expect(horizontalBounds.max.z).toBeCloseTo(3 * (2.5 / 3.4) / 4);
      expect(verticalBounds.min.y + vertical.position.y).toBeCloseTo(-0.25);
      expect(verticalBounds.max.y + vertical.position.y).toBeCloseTo(0.45);
      expect(verticalBounds.min.z).toBeCloseTo(-(0.5 / 0.7) / 4);
      expect(verticalBounds.max.z).toBeCloseTo(3 * (0.5 / 0.7) / 4);
      const span = horizontalBounds.max.x - horizontalBounds.min.x;
      const chord = horizontalBounds.max.z - horizontalBounds.min.z;
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

  it("shows registered preview tails at neutral incidence without changing absent controls", () => {
    const airframe = createBirdmanAirframe();
    try {
      const horizontal = group(airframe, "horizontal-tail-incidence");
      const vertical = group(airframe, "vertical-tail-incidence");
      const meshes = [...surfaces(horizontal), ...surfaces(vertical)];
      expect(horizontal.visible || vertical.visible).toBe(false);
      airframe.setVisualState(null, { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: 0.1, verticalTailRadians: -0.2
      } }, PLAYABLE_GEOMETRY);
      const origins = [horizontal.position.toArray(), vertical.position.toArray()];
      airframe.setVisualState(null, { layout: "registered_preview" });
      expect(horizontal.visible && vertical.visible).toBe(true);
      expect(horizontal.rotation.x).toBe(0);
      expect(vertical.rotation.y).toBe(0);
      expect([horizontal.position.toArray(), vertical.position.toArray()]).toEqual(origins);
      expect([...surfaces(horizontal), ...surfaces(vertical)]).toEqual(meshes);
      airframe.setVisualState(null, NO_AIRFRAME_CONTROLS);
      expect(horizontal.visible || vertical.visible).toBe(false);
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
      const horizontalMeshes = surfaces(horizontal).map((surface) => surface.geometry);
      const verticalMeshes = surfaces(vertical).map((surface) => surface.geometry);
      for (const horizontalTailArmMeters of [3.6, 3.61, 4.2]) {
        airframe.setVisualState(null, controls, { kind: "available", value: { horizontalTailArmMeters } });
        airframe.root.updateMatrixWorld(true);
        expect(horizontal.getWorldPosition(new Vector3()).toArray()).toEqual([0, -0.1, horizontalTailArmMeters]);
        expect(vertical.getWorldPosition(new Vector3()).toArray()).toEqual([0, 0.1, 1.8]);
        expect(horizontal.rotation.x).toBe(controls.physicalIncidence.horizontalTailRadians);
        expect(vertical.rotation.y).toBe(-controls.physicalIncidence.verticalTailRadians);
        for (const [index, surface] of surfaces(horizontal).entries()) expect(surface.geometry).toBe(horizontalMeshes[index]);
        for (const [index, surface] of surfaces(vertical).entries()) expect(surface.geometry).toBe(verticalMeshes[index]);
        airframe.setVisualState(null, NO_AIRFRAME_CONTROLS);
        expect(horizontal.visible || vertical.visible).toBe(false);
      }
      expect(controls.physicalIncidence).toEqual({ horizontalTailRadians: 0.1, verticalTailRadians: -0.2 });
    } finally { airframe.dispose(); }
  });

  it("rejects missing tail metadata before changing the tail pose", () => {
    const airframe = createBirdmanAirframe();
    const controls = { layout: "tail_incidence" as const, physicalIncidence: { horizontalTailRadians: 0.1, verticalTailRadians: -0.2 } };
    try {
      airframe.setVisualState(null, controls, PLAYABLE_GEOMETRY);
        expect(() => { airframe.setVisualState(null, { ...controls, physicalIncidence: { horizontalTailRadians: 0.2, verticalTailRadians: 0.2 } }); }).toThrow("registered aircraft geometry");
        expect(group(airframe, "horizontal-tail-incidence").position.z).toBe(3.6);
        expect(group(airframe, "horizontal-tail-incidence").rotation.x).toBe(0.1);
        expect(group(airframe, "vertical-tail-incidence").rotation.y).toBe(0.2);
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
