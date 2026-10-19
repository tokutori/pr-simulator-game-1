import type { Pose } from "./math.js";

export type FixedCameraPointId = "platform" | "shore" | "telephoto";

export interface FixedCameraPoint {
  readonly id: FixedCameraPointId;
  readonly northMeters: number;
  readonly eastMeters: number;
  readonly altitudeMeters: number;
}

export interface CinematicCameraView {
  readonly pose: Pose;
  readonly verticalFieldOfViewDegrees: number;
}
