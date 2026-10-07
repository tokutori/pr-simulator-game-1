import { inversePose, transformPoint, vec3 } from "./math.js";
import type { Pose, Vec3 } from "./math.js";

export type ProjectionMatrix = readonly [
  number, number, number, number,
  number, number, number, number,
  number, number, number, number,
  number, number, number, number
];

export interface EyeGeometry<Eye extends "left" | "right" = "left" | "right"> {
  readonly eye: Eye;
  readonly headFromEye: Pose;
  readonly projection: ProjectionMatrix;
}

export type BinocularGeometry = readonly [EyeGeometry<"left">, EyeGeometry<"right">];
export type ViewerGeometryUnavailableReason = "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry";

export type ViewerFrame =
  | { readonly source: "unavailable"; readonly reason: ViewerGeometryUnavailableReason; readonly trackingFromHead: Pose | null }
  | { readonly source: "configured"; readonly trackingFromHead: null; readonly eyes: BinocularGeometry }
  | { readonly source: "runtime-derived"; readonly trackingFromHead: Pose; readonly eyes: BinocularGeometry };

export function unavailableViewerFrame(reason: ViewerGeometryUnavailableReason, trackingFromHead: Pose | null = null): ViewerFrame {
  return Object.freeze({ source: "unavailable", reason, trackingFromHead });
}

export function copyProjectionMatrix(values: ArrayLike<number>): ProjectionMatrix | null {
  if (values.length !== 16) return null;
  const copied = Array.from(values);
  if (!copied.every(Number.isFinite)) return null;
  const rows = Array.from({ length: 4 }, (_, row) => Array.from({ length: 4 }, (_, column) => copied[column * 4 + row] ?? 0));
  for (let column = 0; column < 4; column++) {
    let pivot = column;
    for (let row = column + 1; row < 4; row++) {
      if (Math.abs(rows[row]?.[column] ?? 0) > Math.abs(rows[pivot]?.[column] ?? 0)) pivot = row;
    }
    const pivotRow = rows[pivot];
    const originalRow = rows[column];
    if (pivotRow === undefined || originalRow === undefined || !Number.isFinite(pivotRow[column]) || (pivotRow[column] ?? 0) === 0) return null;
    rows[pivot] = originalRow;
    rows[column] = pivotRow;
    for (let row = column + 1; row < 4; row++) {
      const valuesAtRow = rows[row];
      if (valuesAtRow === undefined) return null;
      const ratio = (valuesAtRow[column] ?? 0) / (pivotRow[column] ?? 0);
      for (let entry = column; entry < 4; entry++) valuesAtRow[entry] = (valuesAtRow[entry] ?? 0) - ratio * (pivotRow[entry] ?? 0);
      if (!valuesAtRow.every(Number.isFinite)) return null;
    }
  }
  return Object.freeze(copied) as ProjectionMatrix;
}

export function projectHeadPoint(eye: EyeGeometry, point: Vec3): Readonly<{ x: number; y: number; z: number }> | null {
  const local = transformPoint(inversePose(eye.headFromEye), point);
  const matrix = eye.projection;
  const homogeneous = [local.x, local.y, local.z, 1];
  const projected = Array.from({ length: 4 }, (_, row) => homogeneous.reduce(
    (sum, value, column) => sum + value * (matrix[column * 4 + row] ?? 0), 0
  ));
  const [clipX = Number.NaN, clipY = Number.NaN, clipZ = Number.NaN, clipW = Number.NaN] = projected;
  if (!projected.every(Number.isFinite) || clipW <= 0) return null;
  return Object.freeze({ x: clipX / clipW, y: clipY / clipW, z: clipZ / clipW });
}

export function headPlaneFitsViews(frame: ViewerFrame, headFromPlane: Pose, width: number, height: number, margin = 0): boolean {
  if (frame.source === "unavailable" || ![width, height, margin].every(Number.isFinite) ||
      width <= 0 || height <= 0 || margin < 0 || margin >= 1) return false;
  return frame.eyes.every((eye) => [-1, 1].every((horizontal) => [-1, 1].every((vertical) => {
    const point = transformPoint(headFromPlane, vec3(horizontal * width / 2, vertical * height / 2, 0));
    const projected = projectHeadPoint(eye, point);
    return projected !== null && Math.abs(projected.x) <= 1 - margin && Math.abs(projected.y) <= 1 - margin &&
      projected.z >= -1 && projected.z <= 1;
  })));
}
