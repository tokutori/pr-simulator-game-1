export interface TerrainVertex {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface TerrainPatchBounds {
  readonly minX: number;
  readonly maxX: number;
  readonly minZ: number;
  readonly maxZ: number;
}

type Axis = "x" | "z";

function coordinate(vertex: TerrainVertex, axis: Axis): number {
  return axis === "x" ? vertex.x : vertex.z;
}

function intersection(a: TerrainVertex, b: TerrainVertex, axis: Axis, boundary: number): TerrainVertex {
  const first = coordinate(a, axis);
  const second = coordinate(b, axis);
  const fraction = second === first ? 0 : (boundary - first) / (second - first);
  return {
    x: a.x + (b.x - a.x) * fraction,
    y: a.y + (b.y - a.y) * fraction,
    z: a.z + (b.z - a.z) * fraction
  };
}

function clipHalfPlane(
  polygon: readonly TerrainVertex[],
  axis: Axis,
  boundary: number,
  keepGreater: boolean
): TerrainVertex[] {
  if (polygon.length === 0) return [];
  const result: TerrainVertex[] = [];
  let previous = polygon[polygon.length - 1];
  if (previous === undefined) return result;
  let previousInside = keepGreater ? coordinate(previous, axis) >= boundary : coordinate(previous, axis) <= boundary;
  for (const current of polygon) {
    const currentInside = keepGreater ? coordinate(current, axis) >= boundary : coordinate(current, axis) <= boundary;
    if (currentInside !== previousInside) result.push(intersection(previous, current, axis, boundary));
    if (currentInside) result.push(current);
    previous = current;
    previousInside = currentInside;
  }
  return result;
}

function hasArea(polygon: readonly TerrainVertex[]): boolean {
  if (polygon.length < 3) return false;
  let twiceArea = 0;
  for (let index = 0; index < polygon.length; index++) {
    const current = polygon[index];
    const next = polygon[(index + 1) % polygon.length];
    if (current === undefined || next === undefined) continue;
    twiceArea += current.x * next.z - next.x * current.z;
  }
  return Math.abs(twiceArea) > 1e-8;
}

/** Returns disjoint polygon pieces outside an axis-aligned patch rectangle. */
export function subtractTerrainPatch(
  polygon: readonly TerrainVertex[],
  bounds: TerrainPatchBounds
): readonly (readonly TerrainVertex[])[] {
  let candidate = [...polygon];
  const outside: TerrainVertex[][] = [];
  const cuts: readonly [Axis, number, boolean][] = [
    ["x", bounds.minX, false],
    ["x", bounds.maxX, true],
    ["z", bounds.minZ, false],
    ["z", bounds.maxZ, true]
  ];
  for (const [axis, boundary, outsideGreater] of cuts) {
    const piece = clipHalfPlane(candidate, axis, boundary, outsideGreater);
    if (hasArea(piece)) outside.push(piece);
    candidate = clipHalfPlane(candidate, axis, boundary, !outsideGreater);
    if (!hasArea(candidate)) break;
  }
  return outside;
}
