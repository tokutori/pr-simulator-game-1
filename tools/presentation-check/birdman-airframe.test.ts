import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { BufferAttribute, DataTexture, Group, Mesh, MeshLambertMaterial, Vector3 } from "three";
import { createBirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";
import { CockpitGeometryBridge, WingGeometryBridge } from "../../web/pkg/birdman_game_wasm.js";

describe("synthetic Birdman airframe", () => {
  it("preserves frozen Web cockpit shape and releases its bulk owner immediately", () => {
    const free = vi.spyOn(CockpitGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      expect(free).toHaveBeenCalledTimes(1);
      const frame = airframe.root.getObjectByName("open-cockpit-frame") as Group;
      expect(frame.children).toHaveLength(14);
      expect(airframe.root.children.filter((part) => part.name === "canopy-rim-segment")).toHaveLength(16);
      const firstRod = frame.children[0] as Mesh;
      firstRod.updateMatrix();
      for (const [height, expected] of [[-0.5, [0, -1.17, -1.10]], [0.5, [-0.16, -1.01, -0.75]]] as const) {
        const actual = new Vector3(0, height, 0).applyMatrix4(firstRod.matrix).toArray();
        for (const [index, coordinate] of actual.entries()) expect(coordinate).toBeCloseTo(expected[index] ?? Number.NaN, 11);
      }
      for (const [name, vertices, indices, samples, color, opacity] of [
        ["transparent-canopy", 150, 720, [[0, [0.025, -1.08, -1.10]], [6, [0, -1.045, -1.10]], [125, [0.22, -1.05, -0.31]]], 0xd6e3e5, 0.23],
        ["rear-cockpit-fairing", 125, 576, [[0, [0.22, -1.05, -0.31]], [25, [0.23, -1.04, -0.06]], [106, [0, -0.83, 1.22]]], 0xe1e3db, 1]
      ] as const) {
        const mesh = airframe.root.getObjectByName(name) as Mesh;
        const position = mesh.geometry.getAttribute("position");
        expect(position.count).toBe(vertices);
        expect(mesh.geometry.index?.count).toBe(indices);
        for (const [index, expected] of samples) {
          const actual = [position.getX(index), position.getY(index), position.getZ(index)];
          for (const [axis, coordinate] of actual.entries()) expect(coordinate).toBeCloseTo(expected[axis] ?? Number.NaN, 6);
        }
        expect((mesh.material as MeshLambertMaterial).color.getHex()).toBe(color);
        expect((mesh.material as MeshLambertMaterial).opacity).toBeCloseTo(opacity, 6);
      }
      airframe.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); free.mockRestore(); }
  });

  it("returns cockpit bulk arrays owned independently of the released bridge", () => {
    const bridge = new CockpitGeometryBridge();
    const positions = bridge.surface_positions();
    positions[0] = 100;
    expect(bridge.surface_positions()[0]).toBeCloseTo(0.025, 7);
    const indices = bridge.surface_indices();
    const radii = bridge.rod_radii();
    bridge.free();
    expect(positions).toHaveLength((150 + 125) * 3);
    expect(indices).toHaveLength(720 + 576);
    expect(radii).toHaveLength(30);
    expect(positions[0]).toBe(100);
  });

  it.each([
    { speed: 0,
      skin: [-0.7300000190734863, -0.6242138147354126, -0.6263368129730225, -0.544516384601593, -0.4357530176639557, -0.17252102494239807, -0.217544287443161],
      film: [-0.6263368129730225, -0.6650257706642151, -0.7300000190734863, -0.5733661651611328, -0.4576103985309601, -0.1862454116344452, -0.20988935232162476] },
    { speed: 9.5,
      skin: [-0.7300000190734863, -0.6242138147354126, -0.6263368129730225, -0.4030400216579437, -0.2172664850950241, 0.12519864737987518, 0.08017538487911224],
      film: [-0.6263368129730225, -0.6650257706642151, -0.7300000190734863, -0.4318898022174835, -0.2391238659620285, 0.11147426068782806, 0.0878303200006485] },
    { speed: 13,
      skin: [-0.7300000190734863, -0.6242138147354126, -0.6263368129730225, -0.28177452087402344, -0.029992319643497467, 0.38038694858551025, 0.3353636860847473],
      film: [-0.6263368129730225, -0.6650257706642151, -0.7300000190734863, -0.31062430143356323, -0.05184970051050186, 0.36666256189346313, 0.34301862120628357] }
  ])("matches frozen Web 040d65b5 root/dihedral-break/tip and section samples at $speed m/s", ({ speed, skin, film }) => {
    const airframe = createBirdmanAirframe();
    try {
      airframe.setVisualState(speed, { layout: "absent" });
      const skinZ = [-0.41999998688697815, -0.14252068102359772, 0.14632707834243774, -0.07985156029462814, -0.07939234375953674, -0.08668836951255798, -0.08331162482500076];
      const filmZ = [0.14632707834243774, 0.43023425340652466, 0.7099999785423279, 0.32502543926239014, 0.2273540198802948, 0.1059194877743721, 0.10408051311969757];
      const lateral = [0, 0, 0, 6.258900165557861, 8.345199584960938, 10.431500434875488, 10.431500434875488];
      const sampleIndices = [0, 8, 16, 314, 416, 518, 1045];
      for (const [name, heights, longitudinal] of [["right-wing", skin, skinZ], ["right-trailing-film", film, filmZ]] as const) {
        const mesh = airframe.root.getObjectByName(name) as Mesh;
        const position = mesh.geometry.getAttribute("position");
        expect(position.count).toBe(1054);
        expect(mesh.geometry.index?.count).toBe(6312);
        for (const [sample, index] of sampleIndices.entries()) {
          expect(position.getX(index)).toBeCloseTo(lateral[sample] ?? Number.NaN, 6);
          expect(position.getY(index)).toBeCloseTo(heights[sample] ?? Number.NaN, 6);
          expect(position.getZ(index)).toBeCloseTo(longitudinal[sample] ?? Number.NaN, 6);
        }
      }
    } finally { airframe.dispose(); }
  });

  it("retains frozen Web structural anchors and suppresses small speed updates before releasing the shared owner once", () => {
    const free = vi.spyOn(WingGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      const wing = airframe.root.getObjectByName("left-wing") as Mesh;
      const spar = airframe.root.getObjectByName("main-wing-spar")?.children[0] as Mesh;
      const rib = airframe.root.getObjectByName("left-trailing-rib") as Mesh;
      const foam = airframe.root.getObjectByName("left-styrofoam-rib") as Mesh;
      const stringer = airframe.root.getObjectByName("balsa-stringer") as Mesh;
      const position = wing.geometry.getAttribute("position");
      if (!(position instanceof BufferAttribute)) throw new Error("Main-wing positions require a BufferAttribute");
      const version = position.version;
      airframe.setVisualState(9.52, { layout: "absent" });
      expect(position.version).toBe(version);
      airframe.setVisualState(13, { layout: "absent" });
      for (const [rod, expected] of [
        [spar, [-0.8692916666666666, -0.7141083881506469, -0.12101812510031407]],
        [rib, [-0.45354347826086955, -0.6941843186430233, 0.43420779465398673]],
        [stringer, [-0.8692916666666668, -0.6707112076106959, 0.15796374979937183]]
      ] as const) {
        rod.updateMatrix();
        const endpoint = new Vector3(0, 0.5, 0).applyMatrix4(rod.matrix).toArray();
        for (const [index, coordinate] of endpoint.entries()) expect(coordinate).toBeCloseTo(expected[index] ?? Number.NaN, 11);
      }
      expect(foam.position.x).toBeCloseTo(-0.45354347826086955, 11);
      expect(foam.position.y).toBeCloseTo(0.002499023847814475, 11);
      airframe.dispose();
      airframe.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); free.mockRestore(); }
  });

  it("returns owned bulk arrays that remain valid after update and bridge release", () => {
    const wing = new WingGeometryBridge();
    const reference = wing.surface_positions();
    const initialTip = reference[518 * 3 + 1];
    reference[0] = 100;
    expect(wing.surface_positions()[0]).toBe(-0);
    expect(wing.update(13)).toBe(true);
    const updated = wing.surface_positions();
    wing.free();
    expect(reference[518 * 3 + 1]).toBe(initialTip);
    expect(updated[518 * 3 + 1]).toBeCloseTo(0.38038694858551025, 6);
    expect(updated.length).toBe(4 * 1054 * 3);
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
