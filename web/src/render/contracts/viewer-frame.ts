import { xr_head_plane_fits_json, xr_project_head_point_json, xr_projection_matrix_is_valid } from "../../../pkg/birdman_game_wasm.js";
import type { XrEyeGeometry, XrViewerFrame } from "../../generated/xr-viewer-frame.js";
import type { Pose, Vec3 } from "./math.js";

export type ProjectionMatrix = XrEyeGeometry["projection"];
export type EyeGeometry<Eye extends "left" | "right" = "left" | "right"> = XrEyeGeometry & Readonly<{ eye: Eye }>;
export type BinocularGeometry = Extract<XrViewerFrame, { readonly source: "configured" }>["eyes"];
export type ViewerGeometryUnavailableReason = Extract<XrViewerFrame, { readonly source: "unavailable" }>["reason"];
export type ViewerFrame = XrViewerFrame;

export function unavailableViewerFrame(reason: ViewerGeometryUnavailableReason, trackingFromHead: Pose | null = null): ViewerFrame {
  return Object.freeze({ source: "unavailable", reason, trackingFromHead });
}

export function copyProjectionMatrix(values: ArrayLike<number>): ProjectionMatrix | null {
  if (values.length !== 16) return null;
  const copied = Array.from(values);
  return xr_projection_matrix_is_valid(new Float64Array(copied)) ? Object.freeze(copied) as ProjectionMatrix : null;
}

export function projectHeadPoint(eye: EyeGeometry, point: Vec3): Vec3 | null {
  return JSON.parse(xr_project_head_point_json(JSON.stringify(eye), JSON.stringify(point))) as Vec3 | null;
}

export function headPlaneFitsViews(frame: ViewerFrame, headFromPlane: Pose, width: number, height: number, margin = 0): boolean {
  return xr_head_plane_fits_json(JSON.stringify(frame), JSON.stringify(headFromPlane), width, height, margin);
}
