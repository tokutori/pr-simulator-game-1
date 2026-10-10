import { BufferAttribute, PlaneGeometry } from "three";
import { lakeMeshSpacingDomain } from "./lake-wave-projection.js";

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
