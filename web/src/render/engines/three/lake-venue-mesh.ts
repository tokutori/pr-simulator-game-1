import {
  BufferGeometry,
  CylinderGeometry,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  ShapeUtils,
  Vector2,
} from "three";
import venueFeatures from "../../../../../assets/biwa-venue-features.json";
import { LAUNCH_PLATFORM } from "../../contracts/launch-venue.js";
import { subtractTerrainPatch, type TerrainPatchBounds, type TerrainVertex } from "./terrain-patch-clip.js";

type NorthEast = readonly [north: number, east: number];

interface ShorelineAsset {
  readonly schemaVersion: number;
  readonly shorelinesNorthEastMeters: readonly (readonly NorthEast[])[];
  readonly shorelineLandSideSigns?: readonly (1 | -1)[];
  readonly islands: readonly {
    readonly wayIds?: readonly number[];
    readonly name: string | null;
    readonly ringNorthEastMeters: readonly NorthEast[];
  }[];
}

interface TerrainGrid {
  readonly terrainStepMeters: number;
  readonly northMinMeters: number;
  readonly eastMinMeters: number;
  readonly columns: number;
  readonly rows: number;
  readonly elevationMetersAboveWater: readonly (number | null)[];
  readonly landMask?: readonly boolean[];
  readonly blendWidthMeters?: number;
}

interface TerrainAsset extends TerrainGrid {
  readonly schemaVersion: 1;
  readonly finePatches: readonly (TerrainGrid & { readonly id: string })[];
}

/**
 * Bridge the OSM water edge to the first valid land samples in the AW3D30 DSM.
 * Coastal DSM cells are masked, so the regular terrain mesh can begin tens of
 * metres inland (and several hundred metres inland on the 300 m grid). This
 * collar slopes from below the water plane into the nearest land samples,
 * overlapping the animated water surface so wave troughs cannot expose a
 * background slit along the coastline.
 */
export function shorelineTransitionGeometry(asset: ShorelineAsset, grids: readonly TerrainGrid[]): BufferGeometry {
  const positions: number[] = [];
  const colors: number[] = [];
  const indices: number[] = [];
  const appendUpwardTriangle = (a: number, b: number, c: number): void => {
    const ax = positions[a * 3] ?? 0; const az = positions[a * 3 + 2] ?? 0;
    const bx = positions[b * 3] ?? 0; const bz = positions[b * 3 + 2] ?? 0;
    const cx = positions[c * 3] ?? 0; const cz = positions[c * 3 + 2] ?? 0;
    const normalY = (bz - az) * (cx - ax) - (bx - ax) * (cz - az);
    if (normalY > 1e-8) indices.push(a, b, c);
    else if (normalY < -1e-8) indices.push(a, c, b);
  };
  const patchBounds = grids.slice(1).map((grid) => ({
    minEast: grid.eastMinMeters,
    maxEast: grid.eastMinMeters + (grid.columns - 1) * grid.terrainStepMeters,
    minNorth: grid.northMinMeters,
    maxNorth: grid.northMinMeters + (grid.rows - 1) * grid.terrainStepMeters
  }));
  const orderedGrids = grids.map((grid, index) => ({ grid: terrainGridWithFallback(grid, grids), index }))
    .sort((a, b) => a.grid.terrainStepMeters - b.grid.terrainStepMeters);
  const hasLandMask = orderedGrids.some(({ grid }) => grid.landMask !== undefined);
  const isMappedLand = (north: number, east: number): boolean => {
    for (const { grid } of orderedGrids) {
      const column = Math.round((east - grid.eastMinMeters) / grid.terrainStepMeters);
      const row = Math.round(grid.rows - 1 - (north - grid.northMinMeters) / grid.terrainStepMeters);
      if (column < 0 || column >= grid.columns || row < 0 || row >= grid.rows) continue;
      const mask = grid.landMask?.[row * grid.columns + column];
      if (mask !== undefined) return mask;
    }
    return true;
  };
  const transitionRingCount = 4;
  // Some AW3D30 DSM tiles have broad coastal no-data bands. Search up to eight
  // coarse cells so the shoreline collar reaches rendered land instead of
  // leaving a sky-visible slit between the OSM coast and the first DEM face.
  const maxSearchMeters = Math.min(2_400, Math.max(120, Math.max(...grids.map((grid) => grid.terrainStepMeters)) * 8));
  const searchIncrement = Math.min(15, Math.max(5, Math.min(...grids.map((grid) => grid.terrainStepMeters)) * 0.5));

  // Match the exact two triangles and patch masking used by terrainGeometry.
  // This makes the collar edge land on the rendered DEM surface rather than
  // on an independent nearest-sample/IDW approximation.
  const sampleTerrainSurface = (north: number, east: number): { height: number } | null => {
    for (const { grid, index } of orderedGrids) {
      const columnPosition = (east - grid.eastMinMeters) / grid.terrainStepMeters;
      const rowPosition = grid.rows - 1 - (north - grid.northMinMeters) / grid.terrainStepMeters;
      if (columnPosition < 0 || rowPosition < 0 || columnPosition > grid.columns - 1 || rowPosition > grid.rows - 1) continue;
      const column = Math.min(grid.columns - 2, Math.floor(columnPosition));
      const row = Math.min(grid.rows - 2, Math.floor(rowPosition));
      const u = columnPosition - column;
      const v = rowPosition - row;
      if (index === 0 && patchBounds.some((bounds) => east > bounds.minEast && east < bounds.maxEast &&
        north > bounds.minNorth && north < bounds.maxNorth)) continue;
      const northwest = grid.elevationMetersAboveWater[row * grid.columns + column];
      const northeast = grid.elevationMetersAboveWater[row * grid.columns + column + 1];
      const southwest = grid.elevationMetersAboveWater[(row + 1) * grid.columns + column];
      const southeast = grid.elevationMetersAboveWater[(row + 1) * grid.columns + column + 1];
      if (u + v <= 1 && northwest !== null && northwest !== undefined &&
        southwest !== null && southwest !== undefined && northeast !== null && northeast !== undefined) {
        const height = Math.max(0.35, northwest) * (1 - u - v) + Math.max(0.35, southwest) * v + Math.max(0.35, northeast) * u;
        return { height };
      }
      if (u + v >= 1 && northeast !== null && northeast !== undefined &&
        southwest !== null && southwest !== undefined && southeast !== null && southeast !== undefined) {
        const height = Math.max(0.35, northeast) * (1 - v) + Math.max(0.35, southwest) * (1 - u) +
          Math.max(0.35, southeast) * (u + v - 1);
        return { height };
      }
    }
    return null;
  };

  // A shoreline polyline can contain sharp OSM corners where the smoothed
  // tangent points along the coast instead of inland. If its normal search
  // travels several grid cells, recover the nearest actual land triangle from
  // the explicit mask. This keeps the transition local instead of drawing a
  // kilometre-wide artificial ramp across a flat, low-resolution DSM band.
  const findNearbyLandSurface = (north: number, east: number, allowedRing?: readonly NorthEast[]): {
    north: number; east: number; width: number; surface: { height: number }
  } | null => {
    const containingGrid = orderedGrids.find(({ grid, index }) => {
      const within = north >= grid.northMinMeters && north <= grid.northMinMeters +
        (grid.rows - 1) * grid.terrainStepMeters && east >= grid.eastMinMeters && east <= grid.eastMinMeters +
        (grid.columns - 1) * grid.terrainStepMeters;
      return within && !(index === 0 && patchBounds.some((bounds) => east > bounds.minEast && east < bounds.maxEast &&
        north > bounds.minNorth && north < bounds.maxNorth));
    });
    const searchRadius = Math.min(maxSearchMeters,
      Math.max(600, (containingGrid?.grid.terrainStepMeters ?? maxSearchMeters) * 4));
    const candidates: { north: number; east: number; distance: number }[] = [];
    for (const { grid } of orderedGrids) {
      const rowPosition = grid.rows - 1 - (north - grid.northMinMeters) / grid.terrainStepMeters;
      const columnPosition = (east - grid.eastMinMeters) / grid.terrainStepMeters;
      const rowRadius = Math.ceil(searchRadius / grid.terrainStepMeters);
      const columnRadius = rowRadius;
      const centerRow = Math.round(rowPosition);
      const centerColumn = Math.round(columnPosition);
      for (let row = centerRow - rowRadius; row <= centerRow + rowRadius; row++) {
        if (row < 0 || row >= grid.rows) continue;
        for (let column = centerColumn - columnRadius; column <= centerColumn + columnRadius; column++) {
          if (column < 0 || column >= grid.columns) continue;
          const gridIndex = row * grid.columns + column;
          if (grid.landMask?.[gridIndex] === false || grid.elevationMetersAboveWater[gridIndex] === null) continue;
          const candidateNorth = grid.northMinMeters + (grid.rows - row - 1) * grid.terrainStepMeters;
          const candidateEast = grid.eastMinMeters + column * grid.terrainStepMeters;
          const distance = Math.hypot(candidateNorth - north, candidateEast - east);
          if (distance > 1e-6 && distance <= searchRadius && isMappedLand(candidateNorth, candidateEast) &&
              (allowedRing === undefined || pointInRing(candidateNorth, candidateEast, allowedRing))) {
            candidates.push({ north: candidateNorth, east: candidateEast, distance });
          }
        }
      }
    }
    candidates.sort((a, b) => a.distance - b.distance);
    for (const candidate of candidates) {
      const surface = sampleTerrainSurface(candidate.north, candidate.east);
      if (surface !== null) return { ...candidate, width: candidate.distance, surface };
    }
    return null;
  };

  const appendPath = (points: readonly NorthEast[], closed: boolean, openLandSide = 1): void => {
    if (points.length < 2) return;
    let signedArea = 0;
    if (closed) {
      for (let index = 0; index < points.length; index++) {
        const current = points[index];
        const next = points[(index + 1) % points.length];
        if (current !== undefined && next !== undefined) signedArea += current[1] * next[0] - next[1] * current[0];
      }
    }
    // The left normal points into a counter-clockwise polygon in (east,north)
    // coordinates. OSM inner-ring assembly does not guarantee one direction.
    const landSide = closed ? signedArea < 0 ? -1 : 1 : openLandSide;
    const samples: NorthEast[] = [];
    const segmentCount = closed ? points.length : points.length - 1;
    for (let segment = 0; segment < segmentCount; segment++) {
      const start = points[segment];
      const end = points[(segment + 1) % points.length];
      if (start === undefined || end === undefined) continue;
      const segmentLength = Math.hypot(end[0] - start[0], end[1] - start[1]);
      const subdivisions = Math.max(1, Math.ceil(segmentLength / 30));
      for (let part = 0; part < subdivisions; part++) {
        const fraction = part / subdivisions;
        samples.push([
          start[0] + (end[0] - start[0]) * fraction,
          start[1] + (end[1] - start[1]) * fraction
        ]);
      }
    }
    if (!closed) {
      const last = points[points.length - 1];
      if (last !== undefined) samples.push(last);
    }
    if (samples.length < 2) return;

    const normals: { north: number; east: number }[] = [];
    const transitions: ({ north: number; east: number; width: number; height: number } | null)[] = [];
    const nearbyLandEndpointGuard = 8;
    for (let index = 0; index < samples.length; index++) {
      const point = samples[index];
      const before = samples[index === 0 ? (closed ? samples.length - 1 : 0) : index - 1];
      const after = samples[index === samples.length - 1 ? (closed ? 0 : index) : index + 1];
      if (point === undefined || before === undefined || after === undefined) continue;
      const deltaNorth = after[0] - before[0];
      const deltaEast = after[1] - before[1];
      const tangentLength = Math.hypot(deltaNorth, deltaEast) || 1;
      const landNorth = deltaEast / tangentLength * landSide;
      const landEast = -deltaNorth / tangentLength * landSide;
      normals.push({ north: landNorth, east: landEast });
      const findLand = (side: number): { north: number; east: number; width: number; surface: { height: number } } | null => {
        let width = 8;
        let candidateNorth = point[0] + landNorth * side * width;
        let candidateEast = point[1] + landEast * side * width;
        let surface = isMappedLand(candidateNorth, candidateEast) ? sampleTerrainSurface(candidateNorth, candidateEast) : null;
        while (surface === null && width < maxSearchMeters) {
          width += searchIncrement;
          candidateNorth = point[0] + landNorth * side * width;
          candidateEast = point[1] + landEast * side * width;
          surface = isMappedLand(candidateNorth, candidateEast) ? sampleTerrainSurface(candidateNorth, candidateEast) : null;
        }
        return surface === null ? null : {
          north: landNorth * side,
          east: landEast * side,
          width,
          surface
        };
      };
      const preferredSide = findLand(1);
      // OSM relation fragments can have a locally inverted winding. If the
      // documented land side contains no mapped terrain, let the explicit
      // land mask identify the other side instead of leaving a coastline gap.
      let transition = preferredSide ?? (hasLandMask ? findLand(-1) : null);
      const canRefineTransition = closed || (index >= nearbyLandEndpointGuard &&
        index + nearbyLandEndpointGuard < samples.length);
      const nearbyTransition = canRefineTransition && transition !== null && transition.width >
        Math.max(120, Math.min(...grids.map((grid) => grid.terrainStepMeters)) * 4)
        ? findNearbyLandSurface(point[0], point[1], closed ? points : undefined)
        : null;
      if (nearbyTransition !== null && (transition === null || nearbyTransition.width < transition.width)) {
        transition = {
          north: (nearbyTransition.north - point[0]) / nearbyTransition.width,
          east: (nearbyTransition.east - point[1]) / nearbyTransition.width,
          width: nearbyTransition.width,
          surface: nearbyTransition.surface
        };
      }
      transitions.push(transition === null ? null : {
        north: transition.north,
        east: transition.east,
        width: transition.width,
        height: transition.surface.height
      });
    }

    // Interpolate only short unsupported runs along the same mapped shoreline.
    const maxInterpolatedGapSamples = 8;
    for (let index = 0; index < samples.length; index++) {
      if (transitions[index] !== null) continue;
      let previousDistance = 1;
      let previous: (typeof transitions)[number] | undefined;
      while (previousDistance <= maxInterpolatedGapSamples + 1) {
        const previousIndex = index - previousDistance;
        if (!closed && previousIndex < 0) break;
        previous = transitions[(previousIndex % samples.length + samples.length) % samples.length];
        if (previous !== null && previous !== undefined) break;
        previousDistance++;
      }
      let nextDistance = 1;
      let next: (typeof transitions)[number] | undefined;
      while (nextDistance <= maxInterpolatedGapSamples + 1) {
        const nextIndex = index + nextDistance;
        if (!closed && nextIndex >= samples.length) break;
        next = transitions[nextIndex % samples.length];
        if (next !== null && next !== undefined) break;
        nextDistance++;
      }
      if (previous === null || previous === undefined || next === null || next === undefined ||
          previousDistance + nextDistance - 1 > maxInterpolatedGapSamples) continue;
      const fraction = previousDistance / (previousDistance + nextDistance);
      const normal = normals[index];
      if (normal === undefined) continue;
      const north = closed ? normal.north : previous.north + (next.north - previous.north) * fraction;
      const east = closed ? normal.east : previous.east + (next.east - previous.east) * fraction;
      const length = Math.hypot(north, east);
      if (length < 0.5) continue;
      transitions[index] = {
        north: north / length,
        east: east / length,
        width: previous.width + (next.width - previous.width) * fraction,
        height: previous.height + (next.height - previous.height) * fraction
      };
    }
    if (!closed) {
      const firstValid = transitions.findIndex((transition) => transition !== null);
      if (firstValid >= 0 && firstValid <= maxInterpolatedGapSamples) {
        const reference = transitions[firstValid];
        if (reference !== null && reference !== undefined) {
          for (let index = 0; index < firstValid; index++) {
            const normal = normals[index];
            if (normal !== undefined) transitions[index] = { ...reference, north: normal.north, east: normal.east };
          }
        }
      }
      let lastValid = transitions.length - 1;
      while (lastValid >= 0 && transitions[lastValid] === null) lastValid--;
      if (lastValid >= 0 && transitions.length - 1 - lastValid <= maxInterpolatedGapSamples) {
        const reference = transitions[lastValid];
        if (reference !== null && reference !== undefined) {
          for (let index = lastValid + 1; index < transitions.length; index++) {
            const normal = normals[index];
            if (normal !== undefined) transitions[index] = { ...reference, north: normal.north, east: normal.east };
          }
        }
      }
      const supported = transitions.slice();
      for (let index = 0; index < transitions.length; index++) {
        if (transitions[index] !== null) continue;
        let nearest: (typeof transitions)[number] | undefined;
        for (let distance = 1; distance <= maxInterpolatedGapSamples && nearest === undefined; distance++) {
          for (const candidateIndex of [index - distance, index + distance]) {
            if (candidateIndex < 0 || candidateIndex >= supported.length) continue;
            const candidate = supported[candidateIndex];
            if (candidate !== null && candidate !== undefined) { nearest = candidate; break; }
          }
        }
        const normal = normals[index];
        if (nearest === undefined || nearest === null || normal === undefined) continue;
        const sign = nearest.north * normal.north + nearest.east * normal.east < 0 ? -1 : 1;
        transitions[index] = { ...nearest, north: normal.north * sign, east: normal.east * sign };
      }
    }

    // Interpolated shoreline profiles can land between rendered DSM triangles.
      // Re-sample each outer collar vertex against the exact terrain surface used
      // by terrainGeometry before assigning its height. Extend the edge a few
      // metres into land and place it slightly below that surface: this hides
      // rasterization cracks between the independently triangulated meshes.
    for (let index = 0; index < transitions.length; index++) {
      const transition = transitions[index];
      const point = samples[index];
      if (transition === null || transition === undefined || point === undefined) continue;
      const landOverlapMeters = 5;
      let width = transition.width + landOverlapMeters;
      let north = point[0] + transition.north * width;
      let east = point[1] + transition.east * width;
      let surface = isMappedLand(north, east) ? sampleTerrainSurface(north, east) : null;
      while (surface === null && width < maxSearchMeters) {
        width += searchIncrement;
        north = point[0] + transition.north * width;
        east = point[1] + transition.east * width;
        surface = isMappedLand(north, east) ? sampleTerrainSurface(north, east) : null;
      }
      if (surface !== null) transitions[index] = { ...transition, width, height: surface.height };
      else if (!isMappedLand(point[0] + transition.north * transition.width,
        point[1] + transition.east * transition.width)) transitions[index] = { ...transition, height: -4 };
    }

    const pathVertices: number[][] = [];
    const pathSampleIndices: number[] = [];
    for (let index = 0; index < samples.length; index++) {
      const point = samples[index];
      const transition = transitions[index];
      if (point === undefined || transition === null || transition === undefined) continue;
      // Extend the collar below the still-water datum before it reaches the
      // shoreline. The water shader displaces the visible lake surface, so a
      // shallow edge can separate from the land during wave troughs or at
      // oblique viewing angles. Keep a broad submerged shelf under the lake;
      // it remains hidden by the water material while closing the seam.
      // Keep both lake-side rings below the deepest visible wave trough. The
      // collar should remain a submerged shelf until it reaches land; raising
      // the shoreline ring above the still-water datum exposes a long ramp
      // whenever a wave trough falls below that ring.
      // The outer ring is sampled on the terrain surface, then tucked 15 cm
      // below it. The terrain mesh therefore wins the depth test while the
      // collar remains behind it to close any subpixel seam.
      const targetHeight = transition.height - 0.15;
      const startVertex = positions.length / 3;
      const shoreOverlapMeters = 240;
      const offsets = [-shoreOverlapMeters, 0, transition.width * 0.5, transition.width];
      const heights = [-8, -8, targetHeight * 0.5, targetHeight];
      for (let ring = 0; ring < offsets.length; ring++) {
        const offset = offsets[ring];
        const height = heights[ring];
        if (offset === undefined || height === undefined) continue;
        const northAtOffset = point[0] + transition.north * offset;
        const eastAtOffset = point[1] + transition.east * offset;
        positions.push(eastAtOffset, height, -northAtOffset);
        const tint = terrainColor(ring === offsets.length - 1 ? transition.height : height, northAtOffset, eastAtOffset);
        colors.push(tint[0], tint[1], tint[2]);
      }
      pathVertices.push([startVertex, startVertex + 1, startVertex + 2, startVertex + 3]);
      pathSampleIndices.push(index);
      if (pathVertices.length > 1) {
        const previous = pathVertices[pathVertices.length - 2];
        const current = pathVertices[pathVertices.length - 1];
        if (previous !== undefined && current !== undefined &&
            pathSampleIndices[pathSampleIndices.length - 1] === (pathSampleIndices[pathSampleIndices.length - 2] ?? -2) + 1) {
          for (let ring = 0; ring < transitionRingCount - 1; ring++) {
            const a = previous[ring]; const b = previous[ring + 1];
            const c = current[ring]; const d = current[ring + 1];
            if (a !== undefined && b !== undefined && c !== undefined && d !== undefined) {
              appendUpwardTriangle(a, c, b);
              appendUpwardTriangle(b, c, d);
            }
          }
        }
      }
    }
    if (closed && pathVertices.length > 2) {
      const previous = pathVertices[pathVertices.length - 1];
      const current = pathVertices[0];
      if (previous !== undefined && current !== undefined &&
          pathSampleIndices[pathSampleIndices.length - 1] === samples.length - 1 && pathSampleIndices[0] === 0) {
        for (let ring = 0; ring < transitionRingCount - 1; ring++) {
          const a = previous[ring]; const b = previous[ring + 1];
          const c = current[ring]; const d = current[ring + 1];
          if (a !== undefined && b !== undefined && c !== undefined && d !== undefined) {
            appendUpwardTriangle(a, c, b);
            appendUpwardTriangle(b, c, d);
          }
        }
      }
    }
  };

  for (let index = 0; index < asset.shorelinesNorthEastMeters.length; index++) {
    const line = asset.shorelinesNorthEastMeters[index];
    if (line !== undefined) appendPath(line, false, asset.shorelineLandSideSigns?.[index] ?? 1);
  }
  for (const island of asset.islands) appendPath(island.ringNorthEastMeters, true);
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setAttribute("color", new Float32BufferAttribute(colors, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}

type TerrainTint = readonly [red: number, green: number, blue: number];
type LandCover = Readonly<{ kind: "beach" | "woodland"; ring: readonly (readonly number[])[] }>;
type ShoreStructure = Readonly<{
  kind: "pier" | "quay" | "breakwater";
  closed: boolean;
  northEastMeters: readonly (readonly number[])[];
}>;

const landCover: readonly LandCover[] = venueFeatures.features.flatMap((feature) => {
  if (!feature.closed || (feature.kind !== "beach" && feature.kind !== "woodland")) return [];
  return [{ kind: feature.kind, ring: feature.northEastMeters }];
});

function pointInRing(northMeters: number, eastMeters: number, ring: LandCover["ring"]): boolean {
  let inside = false;
  for (let index = 0, previous = ring.length - 1; index < ring.length; previous = index, index++) {
    const current = ring[index];
    const before = ring[previous];
    if (current === undefined || before === undefined) continue;
    const currentNorth = current[0];
    const currentEast = current[1];
    const beforeNorth = before[0];
    const beforeEast = before[1];
    if (currentNorth === undefined || currentEast === undefined || beforeNorth === undefined || beforeEast === undefined) continue;
    const crosses = (currentNorth > northMeters) !== (beforeNorth > northMeters) &&
      eastMeters < (beforeEast - currentEast) * (northMeters - currentNorth) /
        (beforeNorth - currentNorth) + currentEast;
    if (crosses) inside = !inside;
  }
  return inside;
}

/** Resample a small island outline at half the source step to close sparse DSM holes. */
export function fillIslandTerrainPatch<T extends TerrainGrid>(
  ring: readonly NorthEast[],
  terrain: T
): Omit<T, keyof TerrainGrid> & TerrainGrid {
  const contour = ring.length > 1 && ring[0]?.[0] === ring[ring.length - 1]?.[0] &&
    ring[0]?.[1] === ring[ring.length - 1]?.[1] ? ring.slice(0, -1) : ring;
  if (contour.length < 3) throw new TypeError("Island terrain fill requires a closed outline");
  const source: { north: number; east: number; height: number }[] = [];
  for (let row = 0; row < terrain.rows; row++) {
    for (let column = 0; column < terrain.columns; column++) {
      const index = row * terrain.columns + column;
      const height = terrain.elevationMetersAboveWater[index];
      if (height === null || height === undefined || terrain.landMask?.[index] === false) continue;
      const north = terrain.northMinMeters + (terrain.rows - row - 1) * terrain.terrainStepMeters;
      const east = terrain.eastMinMeters + column * terrain.terrainStepMeters;
      if (pointInRing(north, east, contour)) source.push({ north, east, height });
    }
  }
  if (source.length < 3) throw new TypeError("Island terrain fill requires at least three mapped land elevations");

  const terrainStepMeters = terrain.terrainStepMeters / 2;
  const columns = (terrain.columns - 1) * 2 + 1;
  const rows = (terrain.rows - 1) * 2 + 1;
  const elevationMetersAboveWater: (number | null)[] = Array.from({ length: columns * rows }, () => null);
  const landMask = new Array<boolean>(columns * rows).fill(false);
  for (let row = 0; row < rows; row++) {
    const north = terrain.northMinMeters + (rows - row - 1) * terrainStepMeters;
    for (let column = 0; column < columns; column++) {
      const east = terrain.eastMinMeters + column * terrainStepMeters;
      if (!pointInRing(north, east, contour)) continue;
      const index = row * columns + column;
      landMask[index] = true;
      const closest = source.map((sample) => ({
        height: sample.height,
        distanceSquared: (sample.north - north) ** 2 + (sample.east - east) ** 2
      })).sort((a, b) => a.distanceSquared - b.distanceSquared).slice(0, 4);
      let totalWeight = 0;
      let weightedHeight = 0;
      for (const sample of closest) {
        const weight = 1 / Math.max(sample.distanceSquared, terrainStepMeters ** 2);
        totalWeight += weight;
        weightedHeight += sample.height * weight;
      }
      elevationMetersAboveWater[index] = weightedHeight / totalWeight;
    }
  }
  return { ...terrain, columns, rows, terrainStepMeters, elevationMetersAboveWater, landMask };
}

function terrainColor(heightMeters: number, northMeters: number, eastMeters: number): TerrainTint {
  const base: TerrainTint = heightMeters < 80 ? [0.32, 0.39, 0.32] : heightMeters < 250 ? [0.29, 0.36, 0.31] :
    heightMeters < 600 ? [0.37, 0.42, 0.39] : [0.45, 0.48, 0.45];
  // AW3D30 is a DSM and already contains canopy elevations. OSM vegetation
  // tags only tint the ground; they never add a second tree-height layer.
  const cover = landCover.find((feature) => pointInRing(northMeters, eastMeters, feature.ring));
  if (cover?.kind === "beach") return [0.48, 0.43, 0.32];
  if (cover?.kind === "woodland") return [base[0] * 0.82, base[1] * 0.9, base[2] * 0.78];
  return base;
}

function terrainHeightAt(grid: TerrainGrid, north: number, east: number): number | null {
  const columnPosition = (east - grid.eastMinMeters) / grid.terrainStepMeters;
  const rowPosition = grid.rows - 1 - (north - grid.northMinMeters) / grid.terrainStepMeters;
  if (columnPosition < 0 || rowPosition < 0 || columnPosition > grid.columns - 1 || rowPosition > grid.rows - 1) return null;
  const column = Math.min(grid.columns - 2, Math.floor(columnPosition));
  const row = Math.min(grid.rows - 2, Math.floor(rowPosition));
  const u = columnPosition - column;
  const v = rowPosition - row;
  const northwest = grid.elevationMetersAboveWater[row * grid.columns + column];
  const northeast = grid.elevationMetersAboveWater[row * grid.columns + column + 1];
  const southwest = grid.elevationMetersAboveWater[(row + 1) * grid.columns + column];
  const southeast = grid.elevationMetersAboveWater[(row + 1) * grid.columns + column + 1];
  if (u + v <= 1 && northwest !== null && northwest !== undefined &&
      southwest !== null && southwest !== undefined && northeast !== null && northeast !== undefined) {
    return Math.max(0.35, northwest) * (1 - u - v) + Math.max(0.35, southwest) * v + Math.max(0.35, northeast) * u;
  }
  if (u + v >= 1 && northeast !== null && northeast !== undefined &&
      southwest !== null && southwest !== undefined && southeast !== null && southeast !== undefined) {
    return Math.max(0.35, northeast) * (1 - v) + Math.max(0.35, southwest) * (1 - u) +
      Math.max(0.35, southeast) * (u + v - 1);
  }
  return null;
}

function terrainHeightFromGrids(grids: readonly TerrainGrid[], north: number, east: number): number | null {
  for (const grid of grids) {
    const northMax = grid.northMinMeters + (grid.rows - 1) * grid.terrainStepMeters;
    const eastMax = grid.eastMinMeters + (grid.columns - 1) * grid.terrainStepMeters;
    if (north < grid.northMinMeters || north > northMax || east < grid.eastMinMeters || east > eastMax) continue;
    const height = terrainHeightAt(grid, north, east);
    if (height !== null) return height;
  }
  return null;
}

function terrainGridWithFallback(grid: TerrainGrid, availableGrids: readonly TerrainGrid[]): TerrainGrid {
  const fallbackGrids = availableGrids
    .filter((candidate) => candidate !== grid && candidate.terrainStepMeters > grid.terrainStepMeters)
    .sort((a, b) => a.terrainStepMeters - b.terrainStepMeters);
  const landMask = grid.landMask;
  const elevations = grid.elevationMetersAboveWater.map((height, index) => {
    if (height !== null) return landMask?.[index] === false ? null : height;
    if (landMask?.[index] === false) return null;
    const row = Math.floor(index / grid.columns);
    const column = index % grid.columns;
    const north = grid.northMinMeters + (grid.rows - row - 1) * grid.terrainStepMeters;
    const east = grid.eastMinMeters + column * grid.terrainStepMeters;
    const fallback = terrainHeightFromGrids(fallbackGrids, north, east);
    if (fallback !== null) return fallback;
    if (landMask?.[index] !== true) return null;
    const maxCells = 4;
    const nearby: { height: number; distanceSquared: number }[] = [];
    for (let rowOffset = -maxCells; rowOffset <= maxCells; rowOffset++) {
      const sampleRow = row + rowOffset;
      if (sampleRow < 0 || sampleRow >= grid.rows) continue;
      for (let columnOffset = -maxCells; columnOffset <= maxCells; columnOffset++) {
        const sampleColumn = column + columnOffset;
        if (sampleColumn < 0 || sampleColumn >= grid.columns) continue;
        const sampleIndex = sampleRow * grid.columns + sampleColumn;
        if (landMask[sampleIndex] === false) continue;
        const sampleHeight = grid.elevationMetersAboveWater[sampleIndex];
        if (sampleHeight === null || sampleHeight === undefined) continue;
        const distanceSquared = rowOffset ** 2 + columnOffset ** 2;
        if (distanceSquared === 0 || distanceSquared > maxCells ** 2) continue;
        nearby.push({ height: sampleHeight, distanceSquared });
      }
    }
    nearby.sort((a, b) => a.distanceSquared - b.distanceSquared);
    const samples = nearby.slice(0, 8);
    if (samples.length === 0) return null;
    let totalWeight = 0;
    let weightedHeight = 0;
    for (const sample of samples) {
      const weight = 1 / sample.distanceSquared;
      weightedHeight += sample.height * weight;
      totalWeight += weight;
    }
    return weightedHeight / totalWeight;
  });
  const unchanged = elevations.every((height, index) => height === grid.elevationMetersAboveWater[index]);
  return unchanged ? grid : { ...grid, elevationMetersAboveWater: elevations };
}

function terrainNormalAt(grids: readonly TerrainGrid[], north: number, east: number): readonly [number, number, number] | null {
  const offset = 5;
  const west = terrainHeightFromGrids(grids, north, east - offset);
  const eastHeight = terrainHeightFromGrids(grids, north, east + offset);
  const south = terrainHeightFromGrids(grids, north - offset, east);
  const northHeight = terrainHeightFromGrids(grids, north + offset, east);
  if (west === null || eastHeight === null || south === null || northHeight === null) return null;
  const slopeEast = (eastHeight - west) / (2 * offset);
  const slopeNorth = (northHeight - south) / (2 * offset);
  const magnitude = Math.hypot(slopeEast, 1, slopeNorth);
  return [-slopeEast / magnitude, 1 / magnitude, slopeNorth / magnitude];
}

export function terrainGeometry(
  asset: TerrainGrid,
  higherDetailRegions: readonly TerrainGrid[] = [],
  normalReferenceGrids: readonly TerrainGrid[] = [asset, ...higherDetailRegions]
): BufferGeometry {
  const count = asset.columns * asset.rows;
  if (asset.elevationMetersAboveWater.length !== count) throw new TypeError("Invalid Lake Biwa terrain dimensions");
  const positions = new Array<number>(count * 3);
  const colors = new Array<number>(count * 3);
  const elevations = terrainGridWithFallback(asset, normalReferenceGrids).elevationMetersAboveWater;
  const patchBounds: TerrainPatchBounds[] = higherDetailRegions.map((region) => {
    const northMax = region.northMinMeters + (region.rows - 1) * region.terrainStepMeters;
    const eastMax = region.eastMinMeters + (region.columns - 1) * region.terrainStepMeters;
    return {
      minX: region.eastMinMeters,
      maxX: eastMax,
      minZ: -northMax,
      maxZ: -region.northMinMeters
    };
  });
  for (let row = 0; row < asset.rows; row++) {
    for (let column = 0; column < asset.columns; column++) {
      const index = row * asset.columns + column;
      const height = elevations[index];
      const north = asset.northMinMeters + (asset.rows - row - 1) * asset.terrainStepMeters;
      const east = asset.eastMinMeters + column * asset.terrainStepMeters;
      positions[index * 3] = east;
      positions[index * 3 + 1] = height === null || height === undefined ? 0 : Math.max(0.35, height);
      positions[index * 3 + 2] = -north;
      const tint = terrainColor(height ?? 0, north, east);
      colors[index * 3] = tint[0];
      colors[index * 3 + 1] = tint[1];
      colors[index * 3 + 2] = tint[2];
    }
  }
  const indices: number[] = [];
  const gridVertex = (index: number): TerrainVertex => ({
    x: positions[index * 3] ?? 0,
    y: positions[index * 3 + 1] ?? 0,
    z: positions[index * 3 + 2] ?? 0
  });
  const appendOutsidePatches = (triangleIndices: readonly [number, number, number], regions: readonly TerrainPatchBounds[]): void => {
    const triangle = triangleIndices.map(gridVertex);
    let pieces: readonly (readonly TerrainVertex[])[] = [triangle];
    for (const region of regions) pieces = pieces.flatMap((piece) => subtractTerrainPatch(piece, region));
    for (const piece of pieces) {
      if (piece.length < 3) continue;
      const firstIndex = positions.length / 3;
      for (const vertex of piece) {
        positions.push(vertex.x, vertex.y, vertex.z);
        const tint = terrainColor(vertex.y, -vertex.z, vertex.x);
        colors.push(tint[0], tint[1], tint[2]);
      }
      for (let vertex = 1; vertex < piece.length - 1; vertex++) {
        const a = piece[0];
        const b = piece[vertex];
        const c = piece[vertex + 1];
        if (a === undefined || b === undefined || c === undefined) continue;
        const normalY = (b.z - a.z) * (c.x - a.x) - (b.x - a.x) * (c.z - a.z);
        if (normalY <= 1e-8) continue;
        indices.push(firstIndex, firstIndex + vertex, firstIndex + vertex + 1);
      }
    }
  };
  const appendTriangle = (triangle: readonly [number, number, number], regions: readonly TerrainPatchBounds[]): void => {
    if (regions.length === 0) {
      indices.push(...triangle);
    } else {
      appendOutsidePatches(triangle, regions);
    }
  };
  for (let row = 0; row < asset.rows - 1; row++) {
    for (let column = 0; column < asset.columns - 1; column++) {
      const northwest = row * asset.columns + column;
      const northeast = northwest + 1;
      const southwest = northwest + asset.columns;
      const southeast = southwest + 1;
      const northOfCell = asset.northMinMeters + (asset.rows - row - 1) * asset.terrainStepMeters;
      const southOfCell = northOfCell - asset.terrainStepMeters;
      const westOfCell = asset.eastMinMeters + column * asset.terrainStepMeters;
      const eastOfCell = westOfCell + asset.terrainStepMeters;
      const cellMinZ = -northOfCell;
      const cellMaxZ = -southOfCell;
      const intersectingRegions = patchBounds.filter((region) => region.minX < eastOfCell && region.maxX > westOfCell &&
        region.minZ < cellMaxZ && region.maxZ > cellMinZ);
      // Keep each triangle inside three valid land samples. Do not let a
      // missing water vertex become a zero-height point in the shoreline mesh.
      const isLand = (index: number): boolean => elevations[index] !== null && elevations[index] !== undefined;
      if (isLand(northwest) && isLand(southwest) && isLand(northeast)) {
        appendTriangle([northwest, southwest, northeast], intersectingRegions);
      }
      if (isLand(northeast) && isLand(southwest) && isLand(southeast)) {
        appendTriangle([northeast, southwest, southeast], intersectingRegions);
      }
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setAttribute("color", new Float32BufferAttribute(colors, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  const normals = geometry.getAttribute("normal");
  const sortedNormalGrids = [...normalReferenceGrids].sort((a, b) => a.terrainStepMeters - b.terrainStepMeters);
  const boundaryRegions = higherDetailRegions.length === 0 ? [asset] : higherDetailRegions;
  const boundaryTolerance = 0.02;
  for (let index = 0; index < positions.length / 3; index++) {
    const east = positions[index * 3] ?? 0;
    const north = -(positions[index * 3 + 2] ?? 0);
    const onPatchBoundary = boundaryRegions.some((region) => {
      const northMax = region.northMinMeters + (region.rows - 1) * region.terrainStepMeters;
      const eastMax = region.eastMinMeters + (region.columns - 1) * region.terrainStepMeters;
      return Math.abs(east - region.eastMinMeters) <= boundaryTolerance ||
        Math.abs(east - eastMax) <= boundaryTolerance ||
        Math.abs(north - region.northMinMeters) <= boundaryTolerance ||
        Math.abs(north - northMax) <= boundaryTolerance;
    });
    if (!onPatchBoundary) continue;
    const normal = terrainNormalAt(sortedNormalGrids, north, east);
    if (normal !== null) normals.setXYZ(index, normal[0], normal[1], normal[2]);
  }
  normals.needsUpdate = true;
  geometry.computeBoundingSphere();
  return geometry;
}

/**
 * Fill the wooded Takeshima outline from its OSM shoreline and AW3D30 DSM.
 * The coarse raster has only a few valid land pixels on this small island, so
 * nearest-sample interpolation closes raster holes without adding tree height.
 */
export function takeshimaCanopyGeometry(
  ring: readonly NorthEast[],
  terrain: TerrainGrid
): BufferGeometry {
  const contour = ring.length > 1 && ring[0]?.[0] === ring[ring.length - 1]?.[0] &&
    ring[0]?.[1] === ring[ring.length - 1]?.[1] ? ring.slice(0, -1) : ring;
  if (contour.length < 3) throw new TypeError("Takeshima canopy requires a closed island outline");
  const samples: { north: number; east: number; height: number }[] = [];
  for (let row = 0; row < terrain.rows; row++) {
    for (let column = 0; column < terrain.columns; column++) {
      const height = terrain.elevationMetersAboveWater[row * terrain.columns + column];
      if (height === null || height === undefined || terrain.landMask?.[row * terrain.columns + column] === false) continue;
      samples.push({
        north: terrain.northMinMeters + (terrain.rows - row - 1) * terrain.terrainStepMeters,
        east: terrain.eastMinMeters + column * terrain.terrainStepMeters,
        height
      });
    }
  }
  if (samples.length === 0) throw new TypeError("Takeshima canopy requires valid AW3D30 samples");
  const elevationAt = (north: number, east: number): number => {
    const closest = samples.map((sample) => ({
      distanceSquared: (sample.north - north) ** 2 + (sample.east - east) ** 2,
      height: sample.height
    })).sort((a, b) => a.distanceSquared - b.distanceSquared).slice(0, 4);
    const exact = closest.find((sample) => sample.distanceSquared < 1e-6);
    if (exact !== undefined) return exact.height;
    let weightedHeight = 0;
    let totalWeight = 0;
    for (const sample of closest) {
      const weight = 1 / Math.max(sample.distanceSquared, 225);
      weightedHeight += sample.height * weight;
      totalWeight += weight;
    }
    return totalWeight === 0 ? 0.5 : weightedHeight / totalWeight;
  };
  const points = contour.map(([north, east]) => new Vector2(east, north));
  const triangles = ShapeUtils.triangulateShape(points, []);
  const positions: number[] = [];
  const colors: number[] = [];
  const indices: number[] = [];
  const append = (north: number, east: number, variation: number): number => {
    const height = Math.max(0.5, elevationAt(north, east)) + 0.12;
    const offset = positions.length / 3;
    positions.push(east, height, -north);
    // Deep forest-green canopy with restrained spatial variation; AW3D30
    // already includes canopy height, so this is color-only vegetation art.
    colors.push(0.025 * variation, 0.16 * variation, 0.045 * variation);
    return offset;
  };
  for (const triangle of triangles) {
    const a = triangle[0];
    const b = triangle[1];
    const c = triangle[2];
    if (a === undefined || b === undefined || c === undefined) continue;
    const vertices = [contour[a], contour[b], contour[c]];
    if (vertices.some((point) => point === undefined)) continue;
    const centerNorth = vertices.reduce((sum, point) => sum + (point?.[0] ?? 0), 0) / 3;
    const centerEast = vertices.reduce((sum, point) => sum + (point?.[1] ?? 0), 0) / 3;
    const variation = 0.82 + (Math.abs(Math.sin(centerNorth * 0.017 + centerEast * 0.013)) * 0.26);
    const start = positions.length / 3;
    for (const point of vertices) {
      if (point === undefined) continue;
      append(point[0], point[1], variation * 0.94);
    }
    const center = append(centerNorth, centerEast, variation * 1.05);
    indices.push(start, start + 1, center, start + 1, start + 2, center, start + 2, start, center);
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setAttribute("color", new Float32BufferAttribute(colors, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}

/**
 * Build render-only low-poly tops for the four Okinoshiraishi rocks. Their
 * OSM rings have no usable AW3D30 elevations (the raster cells are fill data),
 * so the municipality's published 14 m maximum is used only as an art scale;
 * the other three peak heights are proportional visual estimates.
 */
export function okinoshiraishiRockGeometry(islands: ShorelineAsset["islands"]): BufferGeometry {
  const wayIds = new Set([41039534, 41039535, 41039553, 41039509]);
  const rocks = islands.filter((island) => island.wayIds?.some((id) => wayIds.has(id)) === true);
  if (rocks.length !== 4) throw new TypeError("Okinoshiraishi requires its four mapped rock outlines");
  const areas = rocks.map(({ ringNorthEastMeters }) => {
    let area = 0;
    for (let index = 0; index < ringNorthEastMeters.length; index++) {
      const current = ringNorthEastMeters[index];
      const next = ringNorthEastMeters[(index + 1) % ringNorthEastMeters.length];
      if (current !== undefined && next !== undefined) area += current[1] * next[0] - next[1] * current[0];
    }
    return Math.abs(area) * 0.5;
  });
  const largestArea = Math.max(...areas);
  const positions: number[] = [];
  const colors: number[] = [];
  const indices: number[] = [];
  for (let rockIndex = 0; rockIndex < rocks.length; rockIndex++) {
    const rock = rocks[rockIndex];
    const area = areas[rockIndex];
    if (rock === undefined || area === undefined || area <= 0) continue;
    const ring = rock.ringNorthEastMeters;
    const centerNorth = ring.reduce((sum, point) => sum + point[0], 0) / ring.length;
    const centerEast = ring.reduce((sum, point) => sum + point[1], 0) / ring.length;
    const peakHeight = 14 * Math.sqrt(area / largestArea);
    const start = positions.length / 3;
    const tint = 0.86 + rockIndex * 0.035;
    // Submerge the shoreline edge to conceal the seam against wave troughs.
    for (const [north, east] of ring) {
      positions.push(east, -1.5, -north);
      colors.push(0.27 * tint, 0.29 * tint, 0.30 * tint);
    }
    const upperStart = positions.length / 3;
    for (let pointIndex = 0; pointIndex < ring.length; pointIndex++) {
      const point = ring[pointIndex];
      if (point === undefined) continue;
      const dx = point[1] - centerEast;
      const dz = point[0] - centerNorth;
      const jitter = 0.86 + 0.10 * Math.sin((pointIndex + 1) * (rockIndex + 2) * 1.73);
      positions.push(centerEast + dx * 0.58 * jitter, peakHeight * (0.52 + 0.04 * Math.sin(pointIndex * 2.1)),
        -(centerNorth + dz * 0.58 * jitter));
      colors.push(0.34 * tint, 0.36 * tint, 0.36 * tint);
    }
    for (let pointIndex = 0; pointIndex < ring.length; pointIndex++) {
      const next = (pointIndex + 1) % ring.length;
      // Two triangles connect the submerged footprint to the raised crown.
      indices.push(start + pointIndex, start + next, upperStart + pointIndex,
        start + next, upperStart + next, upperStart + pointIndex);
    }
    const crown = positions.length / 3;
    positions.push(centerEast, peakHeight, -centerNorth);
    colors.push(0.38 * tint, 0.39 * tint, 0.38 * tint);
    for (let pointIndex = 0; pointIndex < ring.length; pointIndex++) {
      const next = (pointIndex + 1) % ring.length;
      indices.push(upperStart + pointIndex, upperStart + next, crown);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setAttribute("color", new Float32BufferAttribute(colors, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}

function appendRaisedFootprint(
  positions: number[], indices: number[],
  outline: readonly (readonly [north: number, east: number])[], topMeters: number
): void {
  if (outline.length < 3) return;
  const points = outline.map(([north, east]) => new Vector2(east, north));
  const triangles = ShapeUtils.triangulateShape(points, []);
  const topStart = positions.length / 3;
  for (const [north, east] of outline) positions.push(east, topMeters, -north);
  for (const triangle of triangles) {
    const a = triangle[0]; const b = triangle[1]; const c = triangle[2];
    if (a !== undefined && b !== undefined && c !== undefined) indices.push(topStart + a, topStart + b, topStart + c);
  }
  const sideStart = positions.length / 3;
  for (const [north, east] of outline) positions.push(east, 0.25, -north, east, topMeters, -north);
  for (let index = 0; index < outline.length; index++) {
    const next = (index + 1) % outline.length;
    const lower = sideStart + index * 2;
    const upper = lower + 1;
    const nextLower = sideStart + next * 2;
    const nextUpper = nextLower + 1;
    indices.push(lower, nextLower, upper, upper, nextLower, nextUpper);
  }
}

export function shoreStructureGeometry(kind: ShoreStructure["kind"]): BufferGeometry {
  const positions: number[] = [];
  const indices: number[] = [];
  const widthMeters = kind === "pier" ? 3 : kind === "quay" ? 4.5 : 6;
  const topMeters = kind === "breakwater" ? 1.5 : kind === "quay" ? 0.9 : 0.7;
  for (const feature of venueFeatures.features as readonly ShoreStructure[]) {
    if (feature.kind !== kind) continue;
    const path = feature.northEastMeters.flatMap((point) => {
      const north = point[0]; const east = point[1];
      return north === undefined || east === undefined ? [] : [[north, east] as const];
    });
    if (feature.closed) {
      const ring = path.length > 1 && path[0]?.[0] === path[path.length - 1]?.[0] &&
        path[0]?.[1] === path[path.length - 1]?.[1] ? path.slice(0, -1) : path;
      appendRaisedFootprint(positions, indices, ring, topMeters);
      continue;
    }
    for (let index = 0; index < path.length - 1; index++) {
      const start = path[index]; const end = path[index + 1];
      if (start === undefined || end === undefined) continue;
      const deltaEast = end[1] - start[1];
      const deltaSouth = start[0] - end[0];
      const length = Math.hypot(deltaEast, deltaSouth);
      if (length < 0.1) continue;
      const halfWidth = widthMeters / 2;
      const offsetNorth = deltaEast / length * halfWidth;
      const offsetEast = deltaSouth / length * halfWidth;
      appendRaisedFootprint(positions, indices, [
        [start[0] - offsetNorth, start[1] - offsetEast],
        [end[0] - offsetNorth, end[1] - offsetEast],
        [end[0] + offsetNorth, end[1] + offsetEast],
        [start[0] + offsetNorth, start[1] + offsetEast]
      ], topMeters);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}

function parseShoreline(value: unknown): ShorelineAsset {
  if (typeof value !== "object" || value === null || !("schemaVersion" in value) ||
      (value.schemaVersion !== 2 && value.schemaVersion !== 3) ||
      !("shorelinesNorthEastMeters" in value) || !Array.isArray(value.shorelinesNorthEastMeters) ||
      !("islands" in value) || !Array.isArray(value.islands)) {
    throw new TypeError("Invalid Lake Biwa shoreline asset");
  }
  if (value.schemaVersion === 3 && (!("shorelineLandSideSigns" in value) ||
      !Array.isArray(value.shorelineLandSideSigns) ||
      value.shorelineLandSideSigns.length !== value.shorelinesNorthEastMeters.length ||
      value.shorelineLandSideSigns.some((side: unknown) => side !== -1 && side !== 1))) {
    throw new TypeError("Invalid Lake Biwa shoreline land-side metadata");
  }
  return value as ShorelineAsset;
}

function parseTerrain(value: unknown): TerrainAsset {
  if (typeof value !== "object" || value === null || !("schemaVersion" in value) || value.schemaVersion !== 1 ||
      !isTerrainGrid(value) || !("finePatches" in value) || !Array.isArray(value.finePatches) ||
      value.finePatches.some((patch: unknown) => typeof patch !== "object" || patch === null ||
        !("id" in patch) || typeof patch.id !== "string" || !isTerrainGrid(patch))) {
    throw new TypeError("Invalid Lake Biwa terrain asset");
  }
  return value as TerrainAsset;
}

interface LandMaskAsset {
  readonly schemaVersion: 1;
  readonly grids: readonly {
    readonly id: string;
    readonly columns: number;
    readonly rows: number;
    readonly landMaskHex: string;
  }[];
}

function parseLandMask(value: unknown): LandMaskAsset {
  if (typeof value !== "object" || value === null || !("schemaVersion" in value) || value.schemaVersion !== 1 ||
      !("grids" in value) || !Array.isArray(value.grids) || value.grids.some((grid: unknown) =>
        typeof grid !== "object" || grid === null || !("id" in grid) || typeof grid.id !== "string" ||
        !("columns" in grid) || typeof grid.columns !== "number" || !("rows" in grid) || typeof grid.rows !== "number" ||
        !("landMaskHex" in grid) || typeof grid.landMaskHex !== "string")) {
    throw new TypeError("Invalid Lake Biwa land mask asset");
  }
  return value as LandMaskAsset;
}

export function applyLandMask(terrain: TerrainAsset, mask: LandMaskAsset): TerrainAsset {
  const attach = <T extends TerrainGrid>(grid: T, id: string): T => {
    const encoded = mask.grids.find((candidate) => candidate.id === id);
    if (encoded === undefined || encoded.columns !== grid.columns || encoded.rows !== grid.rows ||
        encoded.landMaskHex.length !== Math.ceil(grid.columns * grid.rows / 4) ||
        !/^[0-9a-f]+$/.test(encoded.landMaskHex)) {
      throw new TypeError(`Missing or invalid Lake Biwa land mask grid: ${id}`);
    }
    const landMask = new Array<boolean>(grid.columns * grid.rows);
    for (let index = 0; index < landMask.length; index++) {
      const nibble = Number.parseInt(encoded.landMaskHex[Math.floor(index / 4)] ?? "0", 16);
      landMask[index] = ((nibble >> (index % 4)) & 1) === 1;
    }
    return { ...grid, landMask };
  };
  return {
    ...attach(terrain, "broad-terrain"),
    finePatches: terrain.finePatches.map((patch) => attach(patch, patch.id))
  };
}

function isTerrainGrid(value: object): value is TerrainGrid {
  if (!("columns" in value) || typeof value.columns !== "number" || !Number.isInteger(value.columns) || value.columns < 2 ||
      !("rows" in value) || typeof value.rows !== "number" || !Number.isInteger(value.rows) || value.rows < 2 ||
      !("terrainStepMeters" in value) || typeof value.terrainStepMeters !== "number" ||
      !Number.isFinite(value.terrainStepMeters) || value.terrainStepMeters <= 0 ||
      !("northMinMeters" in value) || typeof value.northMinMeters !== "number" || !Number.isFinite(value.northMinMeters) ||
      !("eastMinMeters" in value) || typeof value.eastMinMeters !== "number" || !Number.isFinite(value.eastMinMeters) ||
      !("elevationMetersAboveWater" in value) || !Array.isArray(value.elevationMetersAboveWater) ||
      value.elevationMetersAboveWater.length !== value.columns * value.rows) return false;
  return value.elevationMetersAboveWater.every((height: unknown) => height === null ||
    (typeof height === "number" && Number.isFinite(height)));
}

export function createLakeVenue(): Readonly<{ group: Group; dispose(): void }> {
  const group = new Group();
  group.name = "lake-biwa-launch-venue";
  const deckMaterial = new MeshBasicMaterial({ color: 0x777b72, side: DoubleSide });
  const supportMaterial = new MeshBasicMaterial({ color: 0x46514c });
  const terrainMaterial = new MeshLambertMaterial({ vertexColors: true, side: DoubleSide });
  const canopyMaterial = new MeshLambertMaterial({ vertexColors: true, side: DoubleSide });
  const rockMaterial = new MeshLambertMaterial({ vertexColors: true, side: DoubleSide });
  const pierMaterial = new MeshLambertMaterial({ color: 0x85765d, side: DoubleSide });
  const quayMaterial = new MeshLambertMaterial({ color: 0x777a73, side: DoubleSide });
  const breakwaterMaterial = new MeshLambertMaterial({ color: 0x696e6a, side: DoubleSide });
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
  void Promise.all([
    fetch(new URL("../../../../../assets/biwa-shoreline.json", import.meta.url)).then((response) => {
      if (!response.ok) throw new Error(`Shoreline HTTP ${String(response.status)}`);
      return response.json() as Promise<unknown>;
    }),
    fetch(new URL("../../../../../assets/biwa-terrain.json", import.meta.url)).then((response) => {
      if (!response.ok) throw new Error(`Terrain HTTP ${String(response.status)}`);
      return response.json() as Promise<unknown>;
    }),
    fetch(new URL("../../../../../assets/biwa-land-mask.json", import.meta.url)).then((response) => {
      if (!response.ok) throw new Error(`Land mask HTTP ${String(response.status)}`);
      return response.json() as Promise<unknown>;
    })
  ]).then(([shoreData, terrainData, landMaskData]) => {
    if (disposed) return;
    const shoreline = parseShoreline(shoreData);
    const terrain = applyLandMask(parseTerrain(terrainData), parseLandMask(landMaskData));
    const sparseIslandFills = [
      { name: "多景島", patchId: "terrain-patch-takeshima" },
      { name: "オコノ洲", patchId: "terrain-patch-okonozu" }
    ].flatMap(({ name, patchId }) => {
      const island = shoreline.islands.find((entry) => entry.name === name);
      const patch = terrain.finePatches.find((entry) => entry.id === patchId);
      return island === undefined || patch === undefined ? [] : [{ patchId, grid: fillIslandTerrainPatch(island.ringNorthEastMeters, patch) }];
    });
    const fineTerrain = terrain.finePatches.map((patch) =>
      sparseIslandFills.find((filled) => filled.patchId === patch.id)?.grid ?? patch
    );
    const terrainGrids = [terrain, ...fineTerrain];
    const shorelineTerrain = new Mesh(shorelineTransitionGeometry(shoreline, terrainGrids), terrainMaterial);
    shorelineTerrain.name = "lake-biwa-shoreline-terrain-transition";
    const distantLand = new Mesh(terrainGeometry(terrain, terrain.finePatches, terrainGrids), terrainMaterial);
    distantLand.name = "lake-biwa-distant-terrain";
    distantLand.receiveShadow = false;
    distantLand.castShadow = false;
    group.add(shorelineTerrain, distantLand);
    const takeshima = shoreline.islands.find((island) => island.name === "多景島");
    const takeshimaPatch = terrain.finePatches.find((patch) => patch.id === "terrain-patch-takeshima");
    if (takeshima !== undefined && takeshimaPatch !== undefined) {
      const canopy = new Mesh(takeshimaCanopyGeometry(takeshima.ringNorthEastMeters, takeshimaPatch), canopyMaterial);
      canopy.name = "lake-biwa-takeshima-wooded-canopy";
      canopy.castShadow = false;
      group.add(canopy);
    }
    for (const [kind, material] of [["pier", pierMaterial], ["quay", quayMaterial], ["breakwater", breakwaterMaterial]] as const) {
      const structures = new Mesh(shoreStructureGeometry(kind), material);
      structures.name = `lake-biwa-${kind}-features-osm`;
      structures.receiveShadow = true;
      structures.castShadow = true;
      group.add(structures);
    }
    for (const patch of fineTerrain) {
      const nestedPatches = fineTerrain.filter((candidate) => candidate.terrainStepMeters < patch.terrainStepMeters);
      const patchTerrain = new Mesh(terrainGeometry(patch, nestedPatches, terrainGrids), terrainMaterial);
      patchTerrain.name = `lake-biwa-${patch.id}`;
      patchTerrain.castShadow = false;
      group.add(patchTerrain);
    }
    const rocks = new Mesh(okinoshiraishiRockGeometry(shoreline.islands), rockMaterial);
    rocks.name = "lake-biwa-okinoshiraishi-rocks-render-only";
    rocks.castShadow = false;
    rocks.receiveShadow = false;
    group.add(rocks);
  }).catch((error: unknown) => { console.warn("Lake Biwa environment could not be loaded", error); });

  return {
    group,
    dispose() {
      disposed = true;
      group.traverse((object) => {
        if (object instanceof Mesh && object.geometry instanceof BufferGeometry) object.geometry.dispose();
      });
      deckMaterial.dispose();
      supportMaterial.dispose();
      terrainMaterial.dispose();
      canopyMaterial.dispose();
      rockMaterial.dispose();
      pierMaterial.dispose();
      quayMaterial.dispose();
      breakwaterMaterial.dispose();
    }
  };
}
