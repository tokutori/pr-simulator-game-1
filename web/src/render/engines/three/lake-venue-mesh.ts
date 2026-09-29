import { BufferGeometry, CylinderGeometry, DoubleSide, Float32BufferAttribute, Group, Mesh, MeshBasicMaterial } from "three";
import { LAUNCH_PLATFORM } from "../../contracts/launch-venue.js";

interface ShorelineAsset {
  readonly schemaVersion: 1;
  readonly shorelineNorthEastMeters: readonly (readonly [number, number])[];
}

function shoreVertices(points: readonly (readonly [number, number])[], widthMeters: number, heightMeters: number): BufferGeometry {
  const positions: number[] = [];
  const indices: number[] = [];
  for (let index = 0; index < points.length; index++) {
    const point = points[index];
    const before = points[Math.max(0, index - 1)];
    const after = points[Math.min(points.length - 1, index + 1)];
    if (point === undefined || before === undefined || after === undefined) continue;
    const deltaNorth = after[0] - before[0];
    const deltaEast = after[1] - before[1];
    const length = Math.hypot(deltaNorth, deltaEast) || 1;
    // Relation 63499's eastern outer boundary runs north to south: its right
    // side is land. Keep the shore at the water datum and extend landward.
    const landNorth = point[0] + widthMeters * deltaEast / length;
    const landEast = point[1] - widthMeters * deltaNorth / length;
    positions.push(point[1], heightMeters, -point[0], landEast, heightMeters, -landNorth);
    if (index > 0) {
      const base = index * 2;
      indices.push(base - 2, base - 1, base, base, base - 1, base + 1);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

function parseShoreline(value: unknown): ShorelineAsset {
  if (typeof value !== "object" || value === null || !("schemaVersion" in value) || value.schemaVersion !== 1 ||
      !("shorelineNorthEastMeters" in value) || !Array.isArray(value.shorelineNorthEastMeters) ||
      value.shorelineNorthEastMeters.length < 2 ||
      value.shorelineNorthEastMeters.some((point: unknown) => !Array.isArray(point) || point.length !== 2 ||
        point.some((coordinate: unknown) => typeof coordinate !== "number" || !Number.isFinite(coordinate)))) {
    throw new TypeError("Invalid Lake Biwa shoreline asset");
  }
  return value as ShorelineAsset;
}

export function createLakeVenue(): Readonly<{ group: Group; dispose(): void }> {
  const group = new Group();
  group.name = "lake-biwa-launch-venue";
  const deckMaterial = new MeshBasicMaterial({ color: 0x777b72, side: DoubleSide });
  const supportMaterial = new MeshBasicMaterial({ color: 0x46514c });
  const landMaterial = new MeshBasicMaterial({ color: 0x65736d, side: DoubleSide });
  const sandMaterial = new MeshBasicMaterial({ color: 0x9a9986, side: DoubleSide });
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
  void fetch(new URL("../../../../../assets/biwa-shoreline.json", import.meta.url))
    .then((response) => {
      if (!response.ok) throw new Error(`Shoreline HTTP ${String(response.status)}`);
      return response.json() as Promise<unknown>;
    })
    .then((data) => {
      if (disposed) return;
      const points = parseShoreline(data).shorelineNorthEastMeters;
      const land = new Mesh(shoreVertices(points, 150, 0.18), landMaterial);
      land.name = "lake-biwa-shore-land";
      const sand = new Mesh(shoreVertices(points, 7, 0.23), sandMaterial);
      sand.name = "lake-biwa-shore-edge";
      group.add(land, sand);
    })
    .catch((error: unknown) => { console.warn("Lake Biwa shoreline could not be loaded", error); });

  return {
    group,
    dispose() {
      disposed = true;
      group.traverse((object) => {
        if (object instanceof Mesh && object.geometry instanceof BufferGeometry) object.geometry.dispose();
      });
      deckMaterial.dispose();
      supportMaterial.dispose();
      landMaterial.dispose();
      sandMaterial.dispose();
    }
  };
}
