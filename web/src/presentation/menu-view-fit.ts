import { composePose, IDENTITY_POSE, inversePose, pose, rotateVec3, transformPoint, vec3 } from "../render/contracts/math.js";
import type { Pose, Vec3 } from "../render/contracts/math.js";
import type { PanelSize } from "../render/contracts/ui.js";
import { headPlaneFitsViews } from "../render/contracts/viewer-frame.js";
import type { EyeGeometry, ViewerFrame, ViewerGeometryUnavailableReason } from "../render/contracts/viewer-frame.js";

export type MenuViewFit =
  | { readonly kind: "ready"; readonly headFromMenu: Pose; readonly distanceMeters: number }
  | { readonly kind: "unavailable"; readonly reason: ViewerGeometryUnavailableReason | "insufficient-view-area" };

export function fitMenuView(
  viewer: ViewerFrame,
  size: PanelSize,
  menuFromPanel: Pose = IDENTITY_POSE,
  minimumDistanceMeters = 2.4,
  clipMargin = 0.035
): MenuViewFit {
  if (![size.width, size.height, minimumDistanceMeters].every((value) => Number.isFinite(value) && value > 0) ||
      !Number.isFinite(clipMargin) || clipMargin < 0 || clipMargin >= 1) {
    throw new RangeError("Menu fit requires positive finite dimensions and a clip margin in [0, 1)");
  }
  if (viewer.source === "unavailable") return Object.freeze({ kind: "unavailable", reason: viewer.reason });
  try {
    return solveMenuView(viewer, size, menuFromPanel, minimumDistanceMeters, clipMargin);
  } catch (error) {
    if (error instanceof RangeError) return Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" });
    throw error;
  }
}

function solveMenuView(
  viewer: Exclude<ViewerFrame, { readonly source: "unavailable" }>,
  size: PanelSize,
  menuFromPanel: Pose,
  minimumDistanceMeters: number,
  clipMargin: number
): MenuViewFit {
  let lower = minimumDistanceMeters;
  let upper = Number.POSITIVE_INFINITY;
  for (const eye of viewer.eyes) {
    const eyeFromHead = inversePose(eye.headFromEye);
    const direction = rotateVec3(eyeFromHead.orientation, vec3(0, 0, -1));
    const projectedDirection = projectHomogeneous(eye, direction, 0);
    for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
      const point = transformPoint(eyeFromHead, transformPoint(menuFromPanel,
        vec3(horizontal * size.width / 2, vertical * size.height / 2, 0)));
      const projectedPoint = projectHomogeneous(eye, point, 1);
      if (![...projectedDirection, ...projectedPoint].every(Number.isFinite)) {
        return Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" });
      }
      const constraints = [
        ...[0, 1].flatMap((coordinate) => [-1, 1].map((sign) => ({
          intercept: (1 - clipMargin) * projectedPoint[3] - sign * (projectedPoint[coordinate] ?? 0),
          slope: (1 - clipMargin) * projectedDirection[3] - sign * (projectedDirection[coordinate] ?? 0)
        }))),
        ...[-1, 1].map((sign) => ({
          intercept: projectedPoint[3] - sign * projectedPoint[2],
          slope: projectedDirection[3] - sign * projectedDirection[2]
        })),
        { intercept: projectedPoint[3], slope: projectedDirection[3] }
      ];
      for (const { intercept, slope } of constraints) {
        if (!Number.isFinite(intercept) || !Number.isFinite(slope)) {
          return Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" });
        }
        if (slope > 0) lower = Math.max(lower, -intercept / slope);
        else if (slope < 0) upper = Math.min(upper, -intercept / slope);
        else if (intercept < 0) return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
      }
    }
  }
  if (!Number.isFinite(lower) || lower > upper) return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
  const distanceMeters = Math.min(upper, lower + Math.max(1, lower) * 1e-10);
  const headFromMenu = pose(vec3(0, 0, -distanceMeters), IDENTITY_POSE.orientation);
  if (!headPlaneFitsViews(viewer, composePose(headFromMenu, menuFromPanel), size.width, size.height, clipMargin)) {
    return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
  }
  return Object.freeze({ kind: "ready", headFromMenu, distanceMeters });
}

function projectHomogeneous(eye: EyeGeometry, point: Vec3, homogeneous: number): readonly [number, number, number, number] {
  const values = [point.x, point.y, point.z, homogeneous];
  const result = Array.from({ length: 4 }, (_unusedEntry, row) => values.reduce(
    (sum, value, column) => sum + value * (eye.projection[column * 4 + row] ?? 0), 0));
  return [result[0] ?? Number.NaN, result[1] ?? Number.NaN, result[2] ?? Number.NaN, result[3] ?? Number.NaN];
}
