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
import { CockpitGeometryBridge, WingGeometryBridge } from "../../../../pkg/birdman_game_wasm.js";

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

function addCockpit(root: Group): void {
  const shared = new CockpitGeometryBridge();
  try {
    const colors = shared.material_colors();
    const opacities = shared.material_opacities();
    const materials = Array.from(colors, (color, index) => {
      const opacity = bulkNumber(opacities, index);
      return new MeshLambertMaterial({ color, opacity, transparent: opacity < 1,
        depthWrite: opacity === 1, ...(index >= 2 ? { side: DoubleSide } : {}) });
    });
    const materialFor = (code: number): MeshLambertMaterial => {
      const material = materials[code];
      if (material === undefined) throw new RangeError("Incomplete shared cockpit palette");
      return material;
    };
    const frame = new Group();
    frame.name = "open-cockpit-frame";
    root.add(frame);
    const endpoints = shared.rod_endpoints();
    const radii = shared.rod_radii();
    const rodMaterials = shared.rod_materials();
    for (let index = 0; index < radii.length; index++) {
      const code = bulkNumber(rodMaterials, index);
      const rod = addRod(code === 0 ? frame : root,
        new Vector3().fromArray(endpoints, index * 6),
        new Vector3().fromArray(endpoints, index * 6 + 3), bulkNumber(radii, index), materialFor(code));
      if (code === 1) rod.name = "canopy-rim-segment";
    }
    const positions = shared.surface_positions();
    const indices = shared.surface_indices();
    const vertexCounts = shared.surface_vertex_counts();
    const indexCounts = shared.surface_index_counts();
    const surfaceMaterials = shared.surface_materials();
    let positionOffset = 0;
    let indexOffset = 0;
    for (let index = 0; index < vertexCounts.length; index++) {
      const positionEnd = positionOffset + bulkNumber(vertexCounts, index) * 3;
      const indexEnd = indexOffset + bulkNumber(indexCounts, index);
      const geometry = new BufferGeometry();
      geometry.setAttribute("position", new Float32BufferAttribute(positions.subarray(positionOffset, positionEnd), 3));
      geometry.setIndex(new BufferAttribute(indices.subarray(indexOffset, indexEnd), 1));
      geometry.computeVertexNormals();
      const mesh = new Mesh(geometry, materialFor(bulkNumber(surfaceMaterials, index)));
      mesh.name = index === 0 ? "transparent-canopy" : "rear-cockpit-fairing";
      if (index === 0) mesh.frustumCulled = false;
      root.add(mesh);
      positionOffset = positionEnd;
      indexOffset = indexEnd;
    }
  } finally { shared.free(); }
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

  const mainBeam = addRod(root,
    new Vector3().fromArray(rodEndpoints, 0), new Vector3(0, -0.57, 4.75), 0.075, carbon);
  mainBeam.name = "single-carbon-main-beam";
  addCockpit(root);

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
