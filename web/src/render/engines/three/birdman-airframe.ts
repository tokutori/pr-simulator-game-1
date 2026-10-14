import {
  BufferAttribute,
  BufferGeometry,
  CylinderGeometry,
  DataTexture,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  LinearFilter,
  LinearMipmapLinearFilter,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  RGBAFormat,
  SRGBColorSpace,
  UnsignedByteType,
  Vector3
} from "three";
import type { PhysicalFlightControls, TailPresentationGeometryAvailability } from "../../contracts/flight-controls.js";
import { WingGeometryBridge } from "../../../../pkg/birdman_game_wasm.js";

function indigoWingTexture(side: -1 | 1, onVisualReady?: () => void): DataTexture {
  const width = 1024;
  const height = 256;
  const pixels = new Uint8Array(width * height * 4);
  const smooth = (from: number, to: number, value: number): number => {
    const t = Math.max(0, Math.min(1, (value - from) / (to - from)));
    return t * t * (3 - 2 * t);
  };
  for (let y = 0; y < height; y++) {
    const chord = (y + 0.5) / height;
    for (let x = 0; x < width; x++) {
      const span = (x + 0.5) / width;
      const grain = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
      const weave = (grain - Math.floor(grain) - 0.5) * 11;
      const dye = 7 * Math.sin(span * 26 + chord * 9 + side) + 4 * Math.sin(span * 87 - chord * 23);
      let white = 0;
      // The photographed resist dye has uneven, joined brush marks rather
      // than a row of equal circles. The two half wings are deliberately different.
      const centers = side < 0 ? [0.08, 0.30, 0.47, 0.74, 0.89] : [0.09, 0.26, 0.54, 0.69, 0.92];
      for (const [ring, center] of centers.entries()) {
        const along = (span - center) / (0.073 + 0.016 * Math.sin(ring * 2.2 + side));
        const across = (chord - 0.26 - 0.055 * Math.sin(ring * 1.7 + side)) / (0.22 - 0.025 * Math.sin(ring * 1.4));
        const angle = Math.atan2(across, along);
        const radius = Math.hypot(along, across)
          + 0.22 * Math.sin(angle * 3 + ring * 1.3)
          + 0.13 * Math.sin(angle * 7 - ring * 0.7)
          + 0.07 * Math.sin(angle * 15 + span * 83);
        const rimWidth = 0.22 + 0.09 * Math.sin(angle * 5 + ring * 2.1);
        const ringBand = smooth(0.80 - rimWidth, 0.80, radius)
          * (1 - smooth(0.84 + rimWidth, 1.00 + rimWidth, radius));
        white = Math.max(white, ringBand);
      }
      const connectingStroke = Math.abs(chord - 0.39 - 0.055 * Math.sin(span * 19 + side * 0.8)
        - 0.025 * Math.sin(span * 47 - side));
      const brushWidth = 0.02 + 0.01 * Math.sin(span * 37 + side);
      white = Math.max(white, (1 - smooth(brushWidth, brushWidth + 0.025, connectingStroke))
        * (1 - smooth(0.62, 0.82, span)));
      white *= 0.82 + 0.18 * Math.sin(span * 147 + chord * 71) * Math.sin(chord * 49 - span * 59);
      const index = (y * width + x) * 4;
      pixels[index] = Math.round((42 + dye + weave) * (1 - white) + (218 + weave * 0.4) * white);
      pixels[index + 1] = Math.round((63 + dye + weave) * (1 - white) + (224 + weave * 0.4) * white);
      pixels[index + 2] = Math.round((86 + dye + weave) * (1 - white) + (218 + weave * 0.4) * white);
      pixels[index + 3] = 255;
    }
  }
  const texture = new DataTexture(pixels, width, height, RGBAFormat, UnsignedByteType);
  texture.colorSpace = SRGBColorSpace;
  texture.magFilter = LinearFilter;
  texture.minFilter = LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.needsUpdate = true;
  if (typeof Image !== "undefined" && typeof document !== "undefined") {
    const decal = new Image();
    decal.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height / 2;
      const context = canvas.getContext("2d");
      if (context === null) return;
      if (side < 0) {
        context.translate(width, 0);
        context.scale(-1, 1);
      }
      // The generated decal has two wing halves and large transparent margins.
      // The photographic strip occupies only part of the dyed leading panel.
      // Preserve its long, slender appearance in the 20.863 m top view.
      context.drawImage(decal, side < 0 ? 32 : 1085, 280, 1051, 148,
        0, 31, width, 66);
      const decalPixels = context.getImageData(0, 0, width, height / 2).data;
      for (let pixel = 0; pixel < decalPixels.length; pixel += 4) {
        const alpha = (decalPixels[pixel + 3] ?? 0) / 255;
        const grain = ((pixel / 4 * 0.61803398875) % 1 - 0.5) * 8;
        pixels[pixel] = Math.round((42 + grain) * (1 - alpha) + 220 * alpha);
        pixels[pixel + 1] = Math.round((63 + grain) * (1 - alpha) + 224 * alpha);
        pixels[pixel + 2] = Math.round((86 + grain) * (1 - alpha) + 218 * alpha);
      }
      texture.needsUpdate = true;
      onVisualReady?.();
    };
    decal.onerror = () => { console.warn("Wing dye decal could not be loaded; using the procedural pattern."); };
    decal.src = new URL("../../../../../assets/indigo-resist-decal.png", import.meta.url).href;
  }
  return texture;
}

export interface BirdmanAirframe {
  readonly root: Group;
  setVisualState(airspeedMetersPerSecond: number | null, controls: AirframeVisualControls, tailGeometry?: TailPresentationGeometryAvailability): void;
  dispose(): void;
}

export type AirframeVisualControls = PhysicalFlightControls | Readonly<{ layout: "absent" }>;
export const NO_AIRFRAME_CONTROLS = Object.freeze({ layout: "absent" } as const);

function bulkNumber(values: Float32Array | Float64Array | Uint32Array, index: number): number {
  const value = values[index];
  if (value === undefined) throw new RangeError("Incomplete shared main-wing bulk output");
  return value;
}

function registeredHorizontalTailArmMeters(kind: string, armMeters: number): number {
  if (kind === "bpg041_playable_version_two" && armMeters === 3.6) return armMeters;
  throw new RangeError("Tail presentation requires registered aircraft geometry");
}

interface CanopyStation {
  readonly z: number;
  readonly centerY: number;
  readonly halfWidth: number;
  readonly halfHeight: number;
}

function canopyPoint(station: CanopyStation, angle: number): Vector3 {
  const rounded = (value: number): number => Math.sign(value) * Math.abs(value) ** 0.8;
  return new Vector3(
    station.halfWidth * rounded(Math.cos(angle)),
    station.centerY + station.halfHeight * rounded(Math.sin(angle)),
    station.z
  );
}

function canopyGeometry(stations: readonly CanopyStation[]): BufferGeometry {
  const circumferenceSteps = 24;
  const rowSize = circumferenceSteps + 1;
  const vertices: number[] = [];
  const indices: number[] = [];
  for (const station of stations) {
    for (let step = 0; step <= circumferenceSteps; step++) {
      vertices.push(...canopyPoint(station, step * Math.PI * 2 / circumferenceSteps).toArray());
    }
  }
  for (let section = 0; section < stations.length - 1; section++) {
    for (let step = 0; step < circumferenceSteps; step++) {
      const a = section * rowSize + step;
      const b = a + rowSize;
      indices.push(a, a + 1, b, a + 1, b + 1, b);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(vertices, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

function pointRod(rod: Mesh, from: Vector3, to: Vector3): void {
  const vector = to.clone().sub(from);
  rod.position.copy(from).add(to).multiplyScalar(0.5);
  rod.scale.y = vector.length();
  rod.quaternion.setFromUnitVectors(new Vector3(0, 1, 0), vector.normalize());
}

function addRod(parent: Group, from: Vector3, to: Vector3, radius: number, material: MeshLambertMaterial): Mesh {
  const rod = new Mesh(new CylinderGeometry(radius, radius, 1, 8), material);
  pointRod(rod, from, to);
  parent.add(rod);
  return rod;
}

export function createBirdmanAirframe(onVisualReady?: () => void): BirdmanAirframe {
  const root = new Group();
  root.name = "birdman-airframe";
  const wingPatterns = {
    left: indigoWingTexture(-1, onVisualReady), right: indigoWingTexture(1, onVisualReady)
  };
  const wingSkins = {
    left: new MeshLambertMaterial({ color: 0xffffff, map: wingPatterns.left, side: DoubleSide }),
    right: new MeshLambertMaterial({ color: 0xffffff, map: wingPatterns.right, side: DoubleSide })
  };
  const whiteWingUnderside = new MeshLambertMaterial({ color: 0xf2f3ed, side: DoubleSide });
  const skin = new MeshLambertMaterial({ color: 0x304c6b, side: DoubleSide });
  const whiteTail = new MeshLambertMaterial({ color: 0xe9ece8, side: DoubleSide });
  const trailingFilm = new MeshLambertMaterial({
    color: 0xd5ddd9, side: DoubleSide, transparent: true, opacity: 0.22, depthWrite: false
  });
  const ribMaterial = new MeshLambertMaterial({ color: 0xcdb483 });
  const styrofoam = new MeshLambertMaterial({ color: 0x77b5d0, side: DoubleSide });
  const carbon = new MeshLambertMaterial({ color: 0x303b3a });
  const whiteFrame = new MeshLambertMaterial({ color: 0xe4e7e3 });
  const canopyFilm = new MeshLambertMaterial({
    color: 0xd6e3e5, side: DoubleSide, transparent: true, opacity: 0.23, depthWrite: false
  });
  const canopyRim = new MeshLambertMaterial({ color: 0xb8c5c7 });
  const fairingSkin = new MeshLambertMaterial({ color: 0xe1e3db, side: DoubleSide });
  const sharedWing = new WingGeometryBridge();
  const positions = sharedWing.surface_positions();
  const uvs = sharedWing.surface_uvs();
  const indices = sharedWing.surface_indices();
  const vertexCount = sharedWing.surface_vertex_count;
  const indexCount = sharedWing.surface_index_count;
  const skinRanges = sharedWing.skin_material_ranges();
  const wings: Mesh<BufferGeometry, MeshLambertMaterial | MeshLambertMaterial[]>[] = [];
  const wingSpar = new Group();
  wingSpar.name = "main-wing-spar";
  root.add(wingSpar);
  for (let surface = 0; surface < 4; surface++) {
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new Float32BufferAttribute(
      positions.subarray(surface * vertexCount * 3, (surface + 1) * vertexCount * 3), 3));
    geometry.setAttribute("uv", new Float32BufferAttribute(
      uvs.subarray(surface * vertexCount * 2, (surface + 1) * vertexCount * 2), 2));
    geometry.setIndex(new BufferAttribute(indices.slice(surface * indexCount, (surface + 1) * indexCount), 1));
    const film = surface % 2 === 1;
    if (!film) {
      for (let range = 0; range < skinRanges.length / 2; range++) {
        geometry.addGroup(bulkNumber(skinRanges, range * 2), bulkNumber(skinRanges, range * 2 + 1), range === 0 ? 0 : 1);
      }
    }
    geometry.computeVertexNormals();
    const material = film ? trailingFilm : [surface < 2 ? wingSkins.left : wingSkins.right, whiteWingUnderside];
    const mesh = new Mesh(geometry, material);
    mesh.name = `${surface < 2 ? "left" : "right"}-${film ? "trailing-film" : "wing"}`;
    mesh.frustumCulled = false;
    root.add(mesh);
    wings.push(mesh);
  }
  const rodKinds = sharedWing.rod_kinds();
  const rodRadii = sharedWing.rod_radii();
  const rodEndpoints = sharedWing.rod_endpoints();
  const rods: Mesh[] = [];
  for (let index = 0; index < rodKinds.length; index++) {
    const kind = bulkNumber(rodKinds, index);
    const rod = addRod(kind === 0 ? wingSpar : root,
      new Vector3().fromArray(rodEndpoints, index * 6),
      new Vector3().fromArray(rodEndpoints, index * 6 + 3),
      bulkNumber(rodRadii, index), kind === 0 ? carbon : ribMaterial);
    rod.name = kind === 0 ? "main-wing-spar-segment" : kind === 2 ? "balsa-stringer" :
      `${bulkNumber(rodEndpoints, index * 6 + 3) < 0 ? "left" : "right"}-trailing-rib`;
    rods.push(rod);
  }
  const foamVertices = sharedWing.foam_vertices();
  const foamIndices = sharedWing.foam_indices();
  const foamPositions = sharedWing.foam_positions();
  const foamCount = foamPositions.length / 3;
  const foamStride = foamVertices.length / foamCount;
  const foamRibs: Mesh[] = [];
  for (let index = 0; index < foamCount; index++) {
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new Float32BufferAttribute(
      foamVertices.subarray(index * foamStride, (index + 1) * foamStride), 3));
    geometry.setIndex(new BufferAttribute(foamIndices.slice(), 1));
    geometry.computeVertexNormals();
    const foam = new Mesh(geometry, styrofoam);
    foam.position.fromArray(foamPositions, index * 3);
    foam.name = `${foam.position.x < 0 ? "left" : "right"}-styrofoam-rib`;
    root.add(foam);
    foamRibs.push(foam);
  }

  const cockpitFrame = new Group();
  cockpitFrame.name = "open-cockpit-frame";
  root.add(cockpitFrame);
  for (const side of [-1, 1] as const) {
    const upperFront = new Vector3(side * 0.16, -1.01, -0.75);
    const upperRear = new Vector3(side * 0.16, -1.01, 0.65);
    const lowerFront = new Vector3(side * 0.15, -1.36, -0.65);
    const lowerRear = new Vector3(side * 0.15, -1.35, 0.48);
    addRod(cockpitFrame, new Vector3(0, -1.17, -1.10), upperFront, 0.012, whiteFrame);
    addRod(cockpitFrame, upperFront, upperRear, 0.012, whiteFrame);
    addRod(cockpitFrame, lowerFront, lowerRear, 0.012, whiteFrame);
    addRod(cockpitFrame, upperFront, lowerFront, 0.008, whiteFrame);
    addRod(cockpitFrame, upperRear, lowerRear, 0.008, whiteFrame);
    addRod(cockpitFrame, lowerFront, upperRear, 0.007, whiteFrame);
  }
  for (const z of [-0.65, 0.48]) {
    addRod(cockpitFrame, new Vector3(-0.15, -1.36, z),
      new Vector3(0.15, -1.36, z), 0.008, whiteFrame);
  }
  const mainBeam = addRod(root,
    new Vector3().fromArray(rodEndpoints, 0), new Vector3(0, -0.57, 4.75), 0.075, carbon);
  mainBeam.name = "single-carbon-main-beam";
  const clearCanopyStations: readonly CanopyStation[] = [
    { z: -1.10, centerY: -1.08, halfWidth: 0.025, halfHeight: 0.035 },
    { z: -1.02, centerY: -1.07, halfWidth: 0.065, halfHeight: 0.10 },
    { z: -0.90, centerY: -1.06, halfWidth: 0.12, halfHeight: 0.17 },
    { z: -0.75, centerY: -1.06, halfWidth: 0.17, halfHeight: 0.22 },
    { z: -0.53, centerY: -1.05, halfWidth: 0.20, halfHeight: 0.25 },
    { z: -0.31, centerY: -1.05, halfWidth: 0.22, halfHeight: 0.27 }
  ];
  const canopy = new Mesh(canopyGeometry(clearCanopyStations), canopyFilm);
  canopy.name = "transparent-canopy";
  canopy.frustumCulled = false;
  root.add(canopy);
  const rearFairing = new Mesh(canopyGeometry([
    { z: -0.31, centerY: -1.05, halfWidth: 0.22, halfHeight: 0.27 },
    { z: -0.06, centerY: -1.04, halfWidth: 0.23, halfHeight: 0.28 },
    { z: 0.30, centerY: -1.03, halfWidth: 0.21, halfHeight: 0.26 },
    { z: 0.77, centerY: -0.98, halfWidth: 0.13, halfHeight: 0.17 },
    { z: 1.22, centerY: -0.87, halfWidth: 0.025, halfHeight: 0.04 }
  ]), fairingSkin);
  rearFairing.name = "rear-cockpit-fairing";
  root.add(rearFairing);
  const frontRim = clearCanopyStations[clearCanopyStations.length - 1];
  if (frontRim !== undefined) {
    for (let step = 0; step < 16; step++) {
      addRod(root,
        canopyPoint(frontRim, step * Math.PI * 2 / 16),
        canopyPoint(frontRim, (step + 1) * Math.PI * 2 / 16),
        0.009, canopyRim);
    }
  }

  const horizontalIncidence = new Group();
  horizontalIncidence.name = "horizontal-tail-incidence";
  horizontalIncidence.position.set(0, -0.1, 3.6);
  const tailChordMeters = 2.5 / 3.4;
  horizontalIncidence.add(rectangularTailSurface([
    [-1.7, 0, -tailChordMeters / 4], [1.7, 0, -tailChordMeters / 4],
    [1.7, 0, 3 * tailChordMeters / 4], [-1.7, 0, 3 * tailChordMeters / 4]
  ], whiteTail));
  root.add(horizontalIncidence);
  const verticalIncidence = new Group();
  verticalIncidence.name = "vertical-tail-incidence";
  verticalIncidence.position.set(0, 0.1, 1.8);
  const finChordMeters = 0.5 / 0.7;
  verticalIncidence.add(rectangularTailSurface([
    [0, -0.35, -finChordMeters / 4], [0, 0.35, -finChordMeters / 4],
    [0, 0.35, 3 * finChordMeters / 4], [0, -0.35, 3 * finChordMeters / 4]
  ], skin));
  root.add(verticalIncidence);

  let disposed = false;
  const setVisualState = (airspeedMetersPerSecond: number | null, controls: AirframeVisualControls, tailGeometry?: TailPresentationGeometryAvailability): void => {
    if (controls.layout === "tail_incidence" &&
        (!Number.isFinite(controls.physicalIncidence.horizontalTailRadians) || !Number.isFinite(controls.physicalIncidence.verticalTailRadians))) {
      throw new RangeError("Physical tail incidence must be finite");
    }
    let horizontalTailArmMeters = 3.6;
    if (controls.layout === "tail_incidence") {
      if (tailGeometry?.kind !== "available") throw new RangeError("Tail presentation requires registered aircraft geometry");
      const geometry = tailGeometry.value;
      horizontalTailArmMeters = registeredHorizontalTailArmMeters(geometry.kind, geometry.horizontalTailArmMeters);
    }
    if (sharedWing.update(airspeedMetersPerSecond ?? Number.NaN)) {
      const nextPositions = sharedWing.surface_positions();
      for (let surface = 0; surface < wings.length; surface++) {
        const wing = wings[surface];
        if (wing === undefined) throw new RangeError("Incomplete main-wing surface projection");
        const position = wing.geometry.getAttribute("position");
        position.array.set(nextPositions.subarray(surface * vertexCount * 3, (surface + 1) * vertexCount * 3));
        position.needsUpdate = true;
        wing.geometry.computeVertexNormals();
      }
      const endpoints = sharedWing.rod_endpoints();
      for (const [index, rod] of rods.entries()) {
        pointRod(rod, new Vector3().fromArray(endpoints, index * 6), new Vector3().fromArray(endpoints, index * 6 + 3));
      }
      const translations = sharedWing.foam_positions();
      for (const [index, foam] of foamRibs.entries()) foam.position.fromArray(translations, index * 3);
    }

    horizontalIncidence.position.z = horizontalTailArmMeters;
    horizontalIncidence.visible = controls.layout === "tail_incidence";
    verticalIncidence.visible = controls.layout === "tail_incidence";
    switch (controls.layout) {
      case "tail_incidence":
        horizontalIncidence.rotation.x = controls.physicalIncidence.horizontalTailRadians;
        verticalIncidence.rotation.y = -controls.physicalIncidence.verticalTailRadians;
        break;
      case "absent":
        horizontalIncidence.rotation.x = 0;
        verticalIncidence.rotation.y = 0;
        break;
    }
  };
  setVisualState(null, NO_AIRFRAME_CONTROLS);
  return {
    root,
    setVisualState,
    dispose() {
      if (disposed) return;
      disposed = true;
      sharedWing.free();
      const geometries = new Set<BufferGeometry>();
      const materials = new Set<MeshLambertMaterial | MeshBasicMaterial>();
      root.traverse((part) => {
        if (part instanceof Mesh) {
          geometries.add(part.geometry as BufferGeometry);
          for (const material of Array.isArray(part.material) ? part.material : [part.material]) {
            materials.add(material as MeshLambertMaterial | MeshBasicMaterial);
          }
        }
      });
      for (const geometry of geometries) geometry.dispose();
      for (const material of materials) material.dispose();
      wingPatterns.left.dispose();
      wingPatterns.right.dispose();
    }
  };
}

function rectangularTailSurface(vertices: readonly (readonly [number, number, number])[], material: MeshLambertMaterial): Mesh {
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(vertices.flat(), 3));
  geometry.setIndex([0, 1, 2, 0, 2, 3]);
  geometry.computeVertexNormals();
  const surface = new Mesh(geometry, material);
  surface.name = "physical-tail-surface";
  return surface;
}
