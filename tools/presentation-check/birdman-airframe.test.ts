import { describe, expect, it } from "vitest";
import { CylinderGeometry, DataTexture, Group, Mesh, MeshLambertMaterial, Object3D, Vector3 } from "three";
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

  it("has finite airfoil geometry and animates only elevator and rudder", () => {
    const airframe = createBirdmanAirframe();
    const left = airframe.root.getObjectByName("left-wing") as Mesh;
    const right = airframe.root.getObjectByName("right-wing") as Mesh;
    const film = airframe.root.getObjectByName("left-trailing-film") as Mesh;
    const underSpar = airframe.root.getObjectByName("left-white-under-spar") as Mesh;
    const rib = airframe.root.getObjectByName("left-trailing-rib") as Mesh;
    const foam = airframe.root.getObjectByName("left-styrofoam-rib") as Mesh;
    const stringer = airframe.root.getObjectByName("balsa-stringer") as Mesh;
    const cockpit = airframe.root.getObjectByName("open-cockpit-frame");
    const canopy = airframe.root.getObjectByName("transparent-canopy") as Mesh;
    const fairing = airframe.root.getObjectByName("rear-cockpit-fairing") as Mesh;
    const mainBeams: Object3D[] = [];
    airframe.root.traverse((part) => {
      if (part.name === "single-carbon-main-beam") mainBeams.push(part);
    });
    const elevatorFilm = airframe.root.getObjectByName("left-elevator-film") as Mesh;
    const tailLeading = airframe.root.getObjectByName("left-tail-leading") as Mesh;
    const tailFoam = airframe.root.getObjectByName("elevator-styrofoam-rib") as Mesh;
    const rudderFoam = airframe.root.getObjectByName("rudder-styrofoam-panel") as Mesh;
    const elevator = airframe.root.getObjectByName("elevator");
    const rudder = airframe.root.getObjectByName("rudder");
    expect(left).toBeInstanceOf(Mesh);
    expect(right).toBeInstanceOf(Mesh);
    expect(film).toBeInstanceOf(Mesh);
    expect(underSpar).toBeInstanceOf(Mesh);
    expect(rib).toBeInstanceOf(Mesh);
    expect(foam).toBeInstanceOf(Mesh);
    expect(stringer).toBeInstanceOf(Mesh);
    const namedCount = (name: string): number => {
      let count = 0;
      airframe.root.traverse((part) => { if (part.name === name) count++; });
      return count;
    };
    expect(namedCount("left-styrofoam-rib")).toBe(22);
    expect(namedCount("right-styrofoam-rib")).toBe(22);
    expect(namedCount("elevator-styrofoam-rib")).toBe(16);
    expect(namedCount("rudder-styrofoam-panel")).toBe(8);
    expect(cockpit).toBeInstanceOf(Group);
    expect(cockpit?.children.length).toBeGreaterThan(8);
    for (const part of cockpit?.children ?? []) {
      expect(((part as Mesh).material as MeshLambertMaterial).color.getHex()).toBe(0xe4e7e3);
    }
    expect(canopy).toBeInstanceOf(Mesh);
    expect(fairing).toBeInstanceOf(Mesh);
    expect(mainBeams).toHaveLength(1);
    const spar = airframe.root.getObjectByName("main-wing-spar") as Group;
    expect(spar).toBeInstanceOf(Group);
    expect(spar.children).toHaveLength(24);
    airframe.root.updateMatrixWorld(true);
    expect(mainBeams[0]?.localToWorld(new Vector3(0, -0.5, 0)).distanceTo(
      (spar.children[0] as Mesh).localToWorld(new Vector3(0, -0.5, 0))
    )).toBeLessThan(1e-5);
    airframe.root.traverse((part) => {
      if (!(part instanceof Mesh) || !(part.geometry instanceof CylinderGeometry)) return;
      if (part.name === "single-carbon-main-beam" || part.parent === spar) return;
      expect(part.geometry.parameters.radiusTop).toBeLessThanOrEqual(0.012);
      expect(part.scale.y).toBeLessThan(2.3);
    });
    expect(elevatorFilm).toBeInstanceOf(Mesh);
    expect(tailLeading).toBeInstanceOf(Mesh);
    expect(tailFoam).toBeInstanceOf(Mesh);
    expect(rudderFoam).toBeInstanceOf(Mesh);
    expect((film.material as MeshLambertMaterial).transparent).toBe(true);
    expect((film.material as MeshLambertMaterial).opacity).toBeLessThan(0.3);
    expect(Array.isArray(left.material)).toBe(true);
    const [wingCover, wingUnderside] = left.material as MeshLambertMaterial[];
    expect(wingUnderside?.map).toBeNull();
    expect(wingUnderside?.color.getHex()).toBe(0xf2f3ed);
    expect(left.geometry.groups.map((group) => group.materialIndex)).toEqual([0, 1, 1]);
    expect(left.geometry.groups[0]?.count).toBe(left.geometry.groups[1]?.count);
    const underSparMaterials = underSpar.material as MeshLambertMaterial[];
    expect(underSparMaterials[0]?.transparent).toBe(true);
    expect(underSparMaterials[1]?.color.getHex()).toBe(0xf2f3ed);
    expect(wingCover?.map).toBeInstanceOf(DataTexture);
    const coverPixels = (wingCover?.map as DataTexture).image.data as Uint8Array;
    let darkPixels = 0;
    let lightPixels = 0;
    for (let pixel = 0; pixel < coverPixels.length; pixel += 4) {
      if ((coverPixels[pixel] ?? 255) < 100) darkPixels++;
      if ((coverPixels[pixel] ?? 0) > 190 && Math.floor(pixel / (1024 * 4)) < 128) lightPixels++;
    }
    expect(darkPixels).toBeGreaterThan(0);
    expect(lightPixels).toBeGreaterThan(0);
    expect((foam.material as MeshLambertMaterial).color.getHex()).toBe(0x77b5d0);
    expect((stringer.material as MeshLambertMaterial).color.getHex()).toBe(0xcdb483);
    expect((canopy.material as MeshLambertMaterial).transparent).toBe(true);
    expect((fairing.material as MeshLambertMaterial).transparent).toBe(false);
    const canopyPositions = canopy.geometry.getAttribute("position");
    for (let station = 1; station < 6; station++) {
      expect(canopyPositions.getX(station * 25)).toBeGreaterThan(
        canopyPositions.getX((station - 1) * 25)
      );
    }
    canopy.geometry.computeBoundingBox();
    fairing.geometry.computeBoundingBox();
    const canopyBounds = canopy.geometry.boundingBox;
    const fairingBounds = fairing.geometry.boundingBox;
    expect(canopyBounds).not.toBeNull();
    expect(fairingBounds).not.toBeNull();
    expect((fairingBounds?.max.z ?? 0) - (canopyBounds?.min.z ?? 0)).toBeCloseTo(2.32, 2);
    expect((fairingBounds?.max.x ?? 0) - (fairingBounds?.min.x ?? 0)).toBeLessThan(0.5);
    expect((fairingBounds?.max.y ?? 0) - (fairingBounds?.min.y ?? 0)).toBeLessThan(0.65);
    expect((elevatorFilm.material as MeshLambertMaterial).transparent).toBe(true);
    expect((tailLeading.material as MeshLambertMaterial).color.getHex()).toBe(0xe9ece8);
    expect(namedCount("tail-fixed-rib")).toBe(16);
    expect(namedCount("left-trailing-rib")).toBe(44);
    expect(airframe.root.getObjectByName("left-tail-film")).toBeInstanceOf(Mesh);
    expect((tailFoam.material as MeshLambertMaterial).color.getHex()).toBe(0x77b5d0);
    expect((rudderFoam.material as MeshLambertMaterial).color.getHex()).toBe(0x77b5d0);
    expect(airframe.root.getObjectByName("elevator-rib")).toBeInstanceOf(Mesh);
    expect(airframe.root.getObjectByName("elevator-balsa-stringer")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-front")).toBeInstanceOf(Mesh);
    const finFront = rudder?.getObjectByName("rudder-front") as Mesh;
    finFront.geometry.computeBoundingBox();
    expect(finFront.geometry.boundingBox?.max.y).toBeCloseTo(1.25, 2);
    const finOutline = finFront.geometry.getAttribute("position");
    expect(finOutline.getZ(1) - finOutline.getZ(0)).toBeCloseTo(0.13, 2);
    expect(finOutline.getZ(3) - finOutline.getZ(0)).toBeCloseTo(0.4, 2);
    expect(rudder?.getObjectByName("rudder-trailing-film")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-rib")).toBeInstanceOf(Mesh);
    expect(rudder?.getObjectByName("rudder-balsa-stringer")).toBeInstanceOf(Mesh);
    expect(airframe.root.getObjectByName("vertical-stabilizer")).toBeUndefined();
    expect(airframe.root.getObjectByName("pilot-head")).toBeUndefined();
    expect(elevator).toBeDefined();
    expect(rudder).toBeDefined();
    const leftPosition = left.geometry.getAttribute("position");
    const rightPosition = right.geometry.getAttribute("position");
    const tipStart = 30 * 17;
    expect(rightPosition.getX(tipStart) - leftPosition.getX(tipStart)).toBeCloseTo(20.863, 3);
    const rootCoveredChord = leftPosition.getZ(16) - leftPosition.getZ(0);
    const tipCoveredChord = leftPosition.getZ(tipStart + 16) - leftPosition.getZ(tipStart);
    expect(rootCoveredChord).toBeGreaterThan(tipCoveredChord * 1.9);
    const wingFilm = airframe.root.getObjectByName("left-trailing-film") as Mesh;
    const filmPosition = wingFilm.geometry.getAttribute("position");
    const rootFilmChord = filmPosition.getZ(16) - filmPosition.getZ(0);
    const underSparPosition = underSpar.geometry.getAttribute("position");
    const rootUnderSparChord = underSparPosition.getZ(16) - underSparPosition.getZ(0);
    expect(rootCoveredChord + rootUnderSparChord + rootFilmChord).toBeCloseTo(1.13, 1);
    const section60 = 18 * 17;
    const section80 = 24 * 17;
    const chordAt = (section: number): number =>
      filmPosition.getZ(section + 16) - leftPosition.getZ(section);
    expect(chordAt(section60)).toBeGreaterThan(0.75);
    expect(chordAt(section60)).toBeLessThan(0.85);
    expect(chordAt(section80)).toBeGreaterThan(0.55);
    expect(chordAt(section80)).toBeLessThan(0.65);
    expect((airframe.root.getObjectByName("rudder") as Group).parent?.position.z).toBeCloseTo(4.25);
    const horizontalTail = airframe.root.getObjectByName("left-tail-leading") as Mesh;
    const finRootZ = (rudder as Group).getWorldPosition(new Vector3()).z;
    const horizontalRootZ = horizontalTail.getWorldPosition(new Vector3()).z;
    expect(horizontalRootZ - finRootZ).toBeCloseTo(0.6);
    const tipVertex = leftPosition.count - 1;
    const initialY = leftPosition.getY(tipVertex);
    const initialRibY = rib.position.y;
    const initialFoamY = foam.position.y;
    airframe.setVisualState(13, 0.12, -0.08);
    expect(leftPosition.getY(tipVertex)).toBeGreaterThan(initialY);
    expect(rib.position.y).toBeGreaterThan(initialRibY);
    expect(foam.position.y).toBeGreaterThan(initialFoamY);
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
