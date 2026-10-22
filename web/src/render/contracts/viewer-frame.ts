import type { XrEyeGeometry, XrViewerFrame } from "../../generated/xr-viewer-frame.js";
import type { Pose } from "./math.js";

export type ProjectionMatrix = XrEyeGeometry["projection"];
export type EyeGeometry<Eye extends "left" | "right" = "left" | "right"> = XrEyeGeometry & Readonly<{ eye: Eye }>;
export type BinocularGeometry = Extract<XrViewerFrame, { readonly source: "configured" }>["eyes"];
export type ViewerGeometryUnavailableReason = Extract<XrViewerFrame, { readonly source: "unavailable" }>["reason"];
export type ViewerFrame = XrViewerFrame;

export function unavailableViewerFrame(reason: ViewerGeometryUnavailableReason, trackingFromHead: Pose | null = null): ViewerFrame {
  return Object.freeze({ source: "unavailable", reason, trackingFromHead });
}
