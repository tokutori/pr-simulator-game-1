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
import { CockpitGeometryBridge, WingGeometryBridge, apply_indigo_wing_decal,
  indigo_wing_texture_size, indigo_wing_textures } from "../../../../pkg/birdman_game_wasm.js";

function indigoWingTextures(onVisualReady?: () => void): Readonly<{
  left: DataTexture; right: DataTexture; dispose(): void;
}> {
  const size = indigo_wing_texture_size();
  const width = bulkNumber(size, 0);
  const height = bulkNumber(size, 1);
  const pixels = indigo_wing_textures();
  const sideBytes = width * height * 4;
  const textures = [0, 1].map((side) => {
    const texture = new DataTexture(pixels.subarray(side * sideBytes, (side + 1) * sideBytes),
      width, height, RGBAFormat, UnsignedByteType);
    texture.colorSpace = SRGBColorSpace;
    texture.magFilter = LinearFilter;
    texture.minFilter = LinearMipmapLinearFilter;
    texture.generateMipmaps = true;
    texture.needsUpdate = true;
    return texture;
  });
  const left = textures[0];
  const right = textures[1];
  if (left === undefined || right === undefined) throw new Error("Incomplete shared wing textures");
  let decal: HTMLImageElement | undefined;
  if (typeof Image !== "undefined" && typeof document !== "undefined") {
    decal = new Image();
    decal.onload = () => {
      if (decal === undefined) return;
      try {
        const canvas = document.createElement("canvas");
        canvas.width = decal.naturalWidth;
        canvas.height = decal.naturalHeight;
        const context = canvas.getContext("2d");
        if (context === null) throw new Error("Wing decal canvas is unavailable");
        context.drawImage(decal, 0, 0);
        const decoded = context.getImageData(0, 0, canvas.width, canvas.height).data;
        const alpha = Uint8Array.from({ length: canvas.width * canvas.height },
          (_, index) => decoded[index * 4 + 3] ?? 0);
        const updated = apply_indigo_wing_decal(pixels, canvas.width, canvas.height, alpha);
        pixels.set(updated);
        left.needsUpdate = true;
        right.needsUpdate = true;
        onVisualReady?.();
      } catch (cause) {
        console.warn("Wing dye decal could not be applied; using the procedural pattern.", cause);
      }
    };
    decal.onerror = () => { console.warn("Wing dye decal could not be loaded; using the procedural pattern."); };
    decal.src = new URL("../../../../../assets/indigo-resist-decal.png", import.meta.url).href;
  }
  return { left, right, dispose() {
    if (decal !== undefined) { decal.onload = null; decal.onerror = null; decal = undefined; }
    left.dispose();
    right.dispose();
  } };
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
  const wingPatterns = indigoWingTextures(onVisualReady);
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
  const sharedWing = new WingGeometryBridge();
  const ribMaterial = new MeshLambertMaterial({ color: sharedWing.rib_color });
  const styrofoam = new MeshLambertMaterial({ color: sharedWing.foam_color, side: DoubleSide });
  const carbon = new MeshLambertMaterial({ color: sharedWing.carbon_color });
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

  const beamEndpoints = sharedWing.main_beam_endpoints();
  const mainBeam = addRod(root,
    new Vector3().fromArray(beamEndpoints, 0), new Vector3().fromArray(beamEndpoints, 3),
    sharedWing.main_beam_radius, carbon);
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
      wingPatterns.dispose();
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
