import { Box3, BufferAttribute, BufferGeometry, PlaneGeometry, Sphere, Vector3 } from "three";
import { lakeMeshSpacingDomain } from "./lake-wave-projection.js";
import type { LakeWaveProjectionInputs } from "./lake-wave-projection.js";

export interface LakeWaterDisplacement {
  readonly horizontal: number;
  readonly vertical: number;
}

export function lakeWaterDisplacementBounds(projection: LakeWaveProjectionInputs): LakeWaterDisplacement {
  const amplitude = projection.waveKAmplitude.slice(0, projection.waveCount)
    .reduce((sum, wave) => sum + Math.abs(wave[3]), 0);
  return { horizontal: 1.5 * Math.abs(projection.choppiness) * amplitude,
    vertical: 1.5 * 1.375 * amplitude + (128 / 255) * (0.6 + 0.55) * 3 };
}

export function createLakeChunks(segments: number, displacement: LakeWaterDisplacement): readonly BufferGeometry[] {
  const source = createLakeGeometry(segments);
  const chunks: BufferGeometry[] = [];
  try {
    for (let row = 0; row < 4; row++) for (let column = 0; column < 4; column++) {
      const firstColumn = Math.floor(column * segments / 4);
      const lastColumn = Math.floor((column + 1) * segments / 4);
      const firstRow = Math.floor(row * segments / 4);
      const lastRow = Math.floor((row + 1) * segments / 4);
      const columns = lastColumn - firstColumn + 1;
      const rows = lastRow - firstRow + 1;
      const geometry = new BufferGeometry();
      chunks.push(geometry);
      for (const [name, attribute] of Object.entries(source.attributes)) {
        const values = new Float32Array(columns * rows * attribute.itemSize);
        for (let localRow = 0; localRow < rows; localRow++) for (let localColumn = 0; localColumn < columns; localColumn++) {
          const sourceIndex = (firstRow + localRow) * (segments + 1) + firstColumn + localColumn;
          const targetIndex = (localRow * columns + localColumn) * attribute.itemSize;
          for (let component = 0; component < attribute.itemSize; component++) {
            values[targetIndex + component] = attribute.getComponent(sourceIndex, component);
          }
        }
        geometry.setAttribute(name, new BufferAttribute(values, attribute.itemSize, attribute.normalized));
      }
      const indices: number[] = [];
      for (let localRow = 0; localRow < rows - 1; localRow++) for (let localColumn = 0; localColumn < columns - 1; localColumn++) {
        const first = localRow * columns + localColumn;
        indices.push(first, first + columns, first + 1, first + columns, first + columns + 1, first + 1);
      }
      geometry.setIndex(indices);
    }
    updateLakeChunkBounds(chunks, displacement);
    return chunks;
  } catch (cause) {
    const failures: unknown[] = [cause];
    try { disposeLakeChunks(chunks); }
    catch (cleanup) { failures.push(cleanup); }
    if (failures.length > 1) throw new AggregateError(failures, "Lake chunk preparation cleanup failed", { cause });
    throw cause;
  } finally { source.dispose(); }
}

export function updateLakeChunkBounds(chunks: readonly BufferGeometry[], displacement: LakeWaterDisplacement): void {
  if (![displacement.horizontal, displacement.vertical].every(value => Number.isFinite(value) && value >= 0)) {
    throw new RangeError("Water displacement bounds must be finite and nonnegative");
  }
  for (const geometry of chunks) {
    const positions = geometry.getAttribute("position");
    const bounds = new Box3();
    const point = new Vector3();
    for (let index = 0; index < positions.count; index++) {
      bounds.expandByPoint(point.set(positions.getX(index), 0, -positions.getY(index)));
    }
    const rounding = 16 * 2 ** -23 * Math.max(1, ...bounds.min.toArray().map(Math.abs), ...bounds.max.toArray().map(Math.abs), displacement.horizontal, displacement.vertical);
    bounds.expandByVector(new Vector3(displacement.horizontal + rounding, displacement.vertical + rounding, displacement.horizontal + rounding));
    geometry.boundingBox = bounds;
    geometry.boundingSphere = bounds.getBoundingSphere(new Sphere());
  }
}

export function disposeLakeChunks(chunks: readonly BufferGeometry[]): void {
  const failures: unknown[] = [];
  for (const geometry of chunks) {
    try { geometry.dispose(); }
    catch (cause) { failures.push(cause); }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length > 1) throw new AggregateError(failures, "Lake chunk disposal failed");
}

export function createLakeGeometry(segments: number): PlaneGeometry {
  lakeMeshSpacingDomain(segments);
  const extent = 6000;
  const geometry = new PlaneGeometry(extent, extent, segments, segments);
  const positions = geometry.getAttribute("position");
  const spacing = new Float32Array(positions.count);
  const halfWidth = extent / 2;
  const exponent = 7.2;
  const denominator = Math.expm1(exponent);
  const map = (coordinate: number): number => Math.sign(coordinate) * halfWidth * Math.expm1(exponent * Math.abs(coordinate) / halfWidth) / denominator;
  const localStep = (coordinate: number): number => extent / segments * exponent * Math.exp(exponent * Math.abs(coordinate) / halfWidth) / denominator;
  for (let index = 0; index < positions.count; index++) {
    const sourceX = positions.getX(index);
    const sourceY = positions.getY(index);
    spacing[index] = Math.max(localStep(sourceX), localStep(sourceY));
    positions.setXY(index, map(sourceX), map(sourceY));
  }
  positions.needsUpdate = true;
  geometry.setAttribute("aGridSpacing", new BufferAttribute(spacing, 1));
  geometry.computeBoundingSphere();
  return geometry;
}
