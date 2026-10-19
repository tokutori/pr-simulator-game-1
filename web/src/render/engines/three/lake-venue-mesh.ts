import {
  BufferAttribute,
  BufferGeometry,
  CylinderGeometry,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
} from "three";
import { ShoreStructureGeometryBridge, VenueTerrainGeometryBridge } from "../../../../pkg/birdman_game_wasm.js";
import { LAUNCH_PLATFORM } from "../../contracts/launch-venue.js";

function projectedNedAttribute(values: Float32Array): Float32BufferAttribute {
  const attribute = new Float32BufferAttribute(values, 3);
  for (let vertex = 0; vertex < attribute.count; vertex++) {
    const north = attribute.getX(vertex);
    const east = attribute.getY(vertex);
    const down = attribute.getZ(vertex);
    attribute.setXYZ(vertex, east, -down, -north);
  }
  return attribute;
}

export function venueTerrainMeshes(): readonly Mesh<BufferGeometry, MeshLambertMaterial>[] {
  const shared = new VenueTerrainGeometryBridge();
  const materials = {
    terrain: new MeshLambertMaterial({ vertexColors: true, side: DoubleSide }),
    canopy: new MeshLambertMaterial({ vertexColors: true, side: DoubleSide }),
    rocks: new MeshLambertMaterial({ vertexColors: true, side: DoubleSide })
  };
  const meshes: Mesh<BufferGeometry, MeshLambertMaterial>[] = [];
  try {
    for (let index = 0; index < shared.mesh_count(); index++) {
      const kind = shared.material_kind(index);
      if (kind !== "terrain" && kind !== "canopy" && kind !== "rocks") {
        throw new RangeError(`Unknown shared terrain material: ${kind}`);
      }
      const mesh = new Mesh(new BufferGeometry(), materials[kind]);
      meshes.push(mesh);
      mesh.name = shared.mesh_name(index);
      mesh.geometry.setAttribute("position", projectedNedAttribute(shared.positions_ned(index)));
      mesh.geometry.setAttribute("normal", projectedNedAttribute(shared.normals_ned(index)));
      mesh.geometry.setAttribute("color", new Float32BufferAttribute(shared.colors_linear(index), 3));
      mesh.geometry.setIndex(new BufferAttribute(shared.indices(index), 1));
      mesh.geometry.computeBoundingSphere();
      mesh.castShadow = false;
      mesh.receiveShadow = false;
    }
    return meshes;
  } catch (error) {
    for (const mesh of meshes) mesh.geometry.dispose();
    for (const material of Object.values(materials)) material.dispose();
    throw error;
  } finally { shared.free(); }
}

export function shoreStructureMesh(kind: "pier" | "quay" | "breakwater"): Mesh<BufferGeometry, MeshLambertMaterial> {
  const shared = new ShoreStructureGeometryBridge(kind);
  const geometry = new BufferGeometry();
  let material: MeshLambertMaterial | null = null;
  try {
    geometry.setAttribute("position", projectedNedAttribute(shared.positions_ned()));
    geometry.setIndex(new BufferAttribute(shared.indices(), 1));
    geometry.computeVertexNormals();
    geometry.computeBoundingSphere();
    material = new MeshLambertMaterial({ color: shared.color_srgb(), side: DoubleSide });
    const mesh = new Mesh(geometry, material);
    mesh.name = `lake-biwa-${kind}-features-osm`;
    mesh.receiveShadow = true;
    mesh.castShadow = true;
    return mesh;
  } catch (error) {
    geometry.dispose();
    material?.dispose();
    throw error;
  } finally { shared.free(); }
}

export function createLakeVenue(): Readonly<{ group: Group; dispose(): void }> {
  const group = new Group();
  group.name = "lake-biwa-launch-venue";
  const deckMaterial = new MeshBasicMaterial({ color: 0x777b72, side: DoubleSide });
  const supportMaterial = new MeshBasicMaterial({ color: 0x46514c });
  const materials = new Set<MeshBasicMaterial | MeshLambertMaterial>([deckMaterial, supportMaterial]);
  const bearing = LAUNCH_PLATFORM.launchBearingDegrees * Math.PI / 180;
  const forwardNorth = Math.cos(bearing);
  const forwardEast = Math.sin(bearing);
  const rightNorth = -forwardEast;
  const rightEast = forwardNorth;
  const rearHeight = LAUNCH_PLATFORM.frontLipAboveWaterMeters +
    LAUNCH_PLATFORM.lengthMeters * Math.tan(LAUNCH_PLATFORM.downwardSlopeDegrees * Math.PI / 180);
  const corners = [
    { along: 0, across: -1, height: LAUNCH_PLATFORM.frontLipAboveWaterMeters },
    { along: 0, across: 1, height: LAUNCH_PLATFORM.frontLipAboveWaterMeters },
    { along: -LAUNCH_PLATFORM.lengthMeters, across: -1, height: rearHeight },
    { along: -LAUNCH_PLATFORM.lengthMeters, across: 1, height: rearHeight }
  ];
  const deckPositions: number[] = [];
  for (const corner of corners) {
    const north = corner.along * forwardNorth + corner.across * LAUNCH_PLATFORM.widthMeters / 2 * rightNorth;
    const east = corner.along * forwardEast + corner.across * LAUNCH_PLATFORM.widthMeters / 2 * rightEast;
    deckPositions.push(east, corner.height, -north);
    const support = new Mesh(new CylinderGeometry(0.12, 0.18, corner.height, 8), supportMaterial);
    support.position.set(east, corner.height / 2, -north);
    group.add(support);
  }
  const deckGeometry = new BufferGeometry();
  deckGeometry.setAttribute("position", new Float32BufferAttribute(deckPositions, 3));
  deckGeometry.setIndex([0, 2, 1, 1, 2, 3]);
  deckGeometry.computeVertexNormals();
  const deck = new Mesh(deckGeometry, deckMaterial);
  deck.name = "sloped-launch-platform";
  group.add(deck);

  let disposed = false;
  void Promise.resolve().then(() => {
    if (disposed) return;
    const terrain = venueTerrainMeshes();
    for (const mesh of terrain) materials.add(mesh.material);
    group.add(...terrain);
    for (const kind of ["pier", "quay", "breakwater"] as const) {
      const structures = shoreStructureMesh(kind);
      materials.add(structures.material);
      group.add(structures);
    }
  }).catch((error: unknown) => { console.warn("Lake Biwa environment could not be loaded", error); });

  return {
    group,
    dispose() {
      disposed = true;
      group.traverse((object) => {
        if (object instanceof Mesh && object.geometry instanceof BufferGeometry) object.geometry.dispose();
      });
      for (const material of materials) material.dispose();
    }
  };
}
