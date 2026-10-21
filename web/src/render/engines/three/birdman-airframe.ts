import {
  BufferAttribute,
  BufferGeometry,
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
  UnsignedByteType
} from "three";
import type { PhysicalFlightControls, TailPresentationGeometryAvailability, WingDeformationAvailability } from "../../contracts/flight-controls.js";
import { AirframeAttachment, AirframeMemberKind, AirframeStructureBridge,
  CockpitGeometryBridge, TailGeometryBridge, WingGeometryBridge, apply_indigo_wing_decal,
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
    disposeResources([() => { left.dispose(); }, () => { right.dispose(); }]);
  } };
}

export interface BirdmanAirframe {
  readonly root: Group;
  setVisualState(deformation: WingDeformationAvailability | null, controls: AirframeVisualControls, tailGeometry?: TailPresentationGeometryAvailability): void;
  dispose(): void;
}

export type AirframeVisualControls = PhysicalFlightControls | Readonly<{ layout: "absent" | "registered_preview" }>;
export const NO_AIRFRAME_CONTROLS = Object.freeze({ layout: "absent" } as const);

function bulkNumber(values: Float32Array | Float64Array | Uint32Array, index: number): number {
  const value = values[index];
  if (value === undefined) throw new RangeError("Incomplete shared airframe bulk output");
  return value;
}

function addCockpit(root: Group): void {
  const shared = new CockpitGeometryBridge();
  try {
    const colors = shared.material_colors();
    const opacities = shared.material_opacities();
    const materials = new Map<number, MeshLambertMaterial>();
    const materialFor = (code: number): MeshLambertMaterial => {
      let material = materials.get(code);
      if (material === undefined) {
        const opacity = bulkNumber(opacities, code);
        material = new MeshLambertMaterial({ color: bulkNumber(colors, code), opacity,
          transparent: opacity < 1, depthWrite: opacity === 1, side: DoubleSide });
        materials.set(code, material);
      }
      return material;
    };
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

function addTails(root: Group): Readonly<{ horizontal: Group; vertical: Group; film: MeshLambertMaterial }> {
  const shared = new TailGeometryBridge();
  try {
    const colors = shared.material_colors();
    const opacities = shared.material_opacities();
    const materials = Array.from(colors, (color, index) => {
      const opacity = bulkNumber(opacities, index);
      return new MeshLambertMaterial({ color, opacity, ...(index === 3 ? {} : { side: DoubleSide }),
        transparent: opacity < 1, depthWrite: opacity === 1 });
    });
    const materialFor = (index: number): MeshLambertMaterial => {
      const material = materials[index];
      if (material === undefined) throw new RangeError("Incomplete shared tail palette");
      return material;
    };
    const origins = shared.origins();
    const tails = ["horizontal-tail-incidence", "vertical-tail-incidence"].map((name, index) => {
      const tail = new Group();
      tail.name = name;
      tail.position.fromArray(origins, index * 3);
      const positions = shared.surface_positions(index);
      const codes = shared.surface_materials(index);
      for (let patch = 0; patch < codes.length; patch++) {
        const vertices = Array.from({ length: 4 }, (_, vertex): readonly [number, number, number] => {
          const start = patch * 12 + vertex * 3;
          return [bulkNumber(positions, start), bulkNumber(positions, start + 1), bulkNumber(positions, start + 2)];
        });
        const code = bulkNumber(codes, patch);
        const mesh = rectangularTailSurface(vertices, materialFor(code));
        mesh.name = code === 0 ? "physical-tail-cover" : code === 1 ? "physical-tail-film" : "tail-foam";
        tail.add(mesh);
      }
      root.add(tail);
      return tail;
    });
    const horizontal = tails[0];
    const vertical = tails[1];
    if (horizontal === undefined || vertical === undefined) throw new RangeError("Incomplete shared tails");
    return { horizontal, vertical, film: materialFor(1) };
  } finally { shared.free(); }
}

const memberNames: Readonly<Record<AirframeMemberKind, string>> = {
  [AirframeMemberKind.Spar]: "spar",
  [AirframeMemberKind.Rib]: "rib",
  [AirframeMemberKind.Stringer]: "stringer",
  [AirframeMemberKind.Foam]: "foam",
  [AirframeMemberKind.MainBeam]: "single-carbon-main-beam",
  [AirframeMemberKind.CockpitFrame]: "open-cockpit-frame",
  [AirframeMemberKind.CanopyRim]: "canopy-rim"
};

function addStructure(root: Group, horizontal: Group, vertical: Group): Readonly<{
  source: AirframeStructureBridge;
  meshes: readonly Readonly<{ index: number; mesh: Mesh; attachment: AirframeAttachment }>[];
}> {
  const source = new AirframeStructureBridge();
  const materials = new Map<number, MeshLambertMaterial>();
  const meshes = Array.from({ length: source.mesh_count }, (_, index) => {
    const attachment = source.attachment(index);
    const kind = source.member_kind(index);
    const color = source.color(index);
    let material = materials.get(color);
    if (material === undefined) {
      material = new MeshLambertMaterial({ color, side: DoubleSide });
      materials.set(color, material);
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new Float32BufferAttribute(source.positions(index), 3));
    geometry.setIndex(new BufferAttribute(source.indices(index), 1));
    geometry.computeVertexNormals();
    const mesh = new Mesh(geometry, material);
    mesh.frustumCulled = false;
    const name = memberNames[kind];
    switch (attachment) {
      case AirframeAttachment.Rigid:
        mesh.name = name;
        root.add(mesh);
        break;
      case AirframeAttachment.MainWing:
        mesh.name = `main-wing-${name}`;
        root.add(mesh);
        break;
      case AirframeAttachment.HorizontalTail:
        mesh.name = `horizontal-tail-${name}`;
        horizontal.add(mesh);
        break;
      case AirframeAttachment.VerticalTail:
        mesh.name = `vertical-tail-${name}`;
        vertical.add(mesh);
        break;
    }
    return { index, mesh, attachment };
  });
  return { source, meshes };
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
  const tails = addTails(root);
  const trailingFilm = tails.film;
  const sharedWing = new WingGeometryBridge();
  const structure = addStructure(root, tails.horizontal, tails.vertical);
  const positions = sharedWing.surface_positions();
  const uvs = sharedWing.surface_uvs();
  const indices = sharedWing.surface_indices();
  const vertexCount = sharedWing.surface_vertex_count;
  const indexCount = sharedWing.surface_index_count;
  const skinRanges = sharedWing.skin_material_ranges();
  const wings: Mesh<BufferGeometry, MeshLambertMaterial | MeshLambertMaterial[]>[] = [];
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
  addCockpit(root);

  const horizontalIncidence = tails.horizontal;
  const verticalIncidence = tails.vertical;
  const registeredHorizontalArm = horizontalIncidence.position.z;

  let disposed = false;
  let projectionFailed = false;
  const setWingVisible = (visible: boolean): void => {
    for (const wing of wings) wing.visible = visible;
    for (const part of structure.meshes) {
      if (part.attachment === AirframeAttachment.MainWing) part.mesh.visible = visible;
    }
  };
  const setVisualState = (deformation: WingDeformationAvailability | null, controls: AirframeVisualControls, tailGeometry?: TailPresentationGeometryAvailability): void => {
    if (controls.layout === "tail_incidence" &&
        (!Number.isFinite(controls.physicalIncidence.horizontalTailRadians) || !Number.isFinite(controls.physicalIncidence.verticalTailRadians))) {
      setWingVisible(false);
      throw new RangeError("Physical tail incidence must be finite");
    }
    let horizontalTailArmMeters = registeredHorizontalArm;
    if (controls.layout === "tail_incidence") {
      if (tailGeometry === undefined) {
        setWingVisible(false);
        throw new RangeError("Tail presentation requires registered aircraft geometry");
      }
      if (tailGeometry.kind === "available") horizontalTailArmMeters = tailGeometry.value.horizontalTailArmMeters;
    }
    root.visible = !(deformation?.kind === "unavailable" && deformation.reason === "unregistered_aircraft_geometry") &&
      tailGeometry?.kind !== "unavailable";
    if (!root.visible) {
      setWingVisible(false);
      horizontalIncidence.visible = false;
      verticalIncidence.visible = false;
      return;
    }
    try {
      if (deformation?.kind !== "unavailable") {
        const updated = sharedWing.update_factor(deformation?.factor ?? 1);
        const structureUpdated = structure.source.update_factor(deformation?.factor ?? 1);
        if (updated || projectionFailed) {
          const nextPositions = sharedWing.surface_positions();
          for (let surface = 0; surface < wings.length; surface++) {
            const wing = wings[surface];
            if (wing === undefined) throw new RangeError("Incomplete main-wing surface projection");
            updateMeshPositions(wing, nextPositions.subarray(surface * vertexCount * 3, (surface + 1) * vertexCount * 3));
          }
        }
        if (structureUpdated || projectionFailed) {
          for (const part of structure.meshes) {
            if (part.attachment === AirframeAttachment.MainWing) {
              updateMeshPositions(part.mesh, structure.source.positions(part.index));
            }
          }
        }
        projectionFailed = false;
      }
    } catch (cause) {
      projectionFailed = true;
      setWingVisible(false);
      throw cause;
    }
    setWingVisible(deformation?.kind !== "unavailable");

    horizontalIncidence.position.z = horizontalTailArmMeters;
    const tailVisible = (controls.layout === "tail_incidence" && tailGeometry?.kind === "available") || controls.layout === "registered_preview";
    horizontalIncidence.visible = tailVisible;
    verticalIncidence.visible = tailVisible;
    switch (controls.layout) {
      case "tail_incidence":
        horizontalIncidence.rotation.x = controls.physicalIncidence.horizontalTailRadians;
        verticalIncidence.rotation.y = -controls.physicalIncidence.verticalTailRadians;
        break;
      case "absent":
      case "registered_preview":
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
      disposeResources([
        () => { sharedWing.free(); },
        () => { structure.source.free(); },
        ...Array.from(geometries, (geometry) => () => { geometry.dispose(); }),
        ...Array.from(materials, (material) => () => { material.dispose(); }),
        () => { wingPatterns.dispose(); }
      ]);
    }
  };
}

function updateMeshPositions(mesh: Mesh, values: Float32Array): void {
  const position = mesh.geometry.getAttribute("position");
  position.array.set(values);
  mesh.geometry.computeVertexNormals();
  const normals = mesh.geometry.getAttribute("normal");
  for (let index = 0; index < normals.count; index++) {
    const length = Math.hypot(normals.getX(index), normals.getY(index), normals.getZ(index));
    if (!Number.isFinite(length) || length < 0.5) throw new RangeError("Wing normals are unrepresentable");
  }
  position.needsUpdate = true;
}

function disposeResources(releases: readonly (() => void)[]): void {
  const failures: unknown[] = [];
  for (const release of releases) {
    try { release(); } catch (cause) { failures.push(cause); }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length > 1) throw new AggregateError(failures, "Airframe resource cleanup failed", { cause: failures[0] });
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
