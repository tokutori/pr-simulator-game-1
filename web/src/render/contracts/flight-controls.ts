import type { FlightRenderPose } from "../../generated/flight-render-pose.js";

export type PhysicalFlightControls = FlightRenderPose["controls"];
export type TailPhysicalFlightControls = PhysicalFlightControls;
export type TailPresentationGeometryAvailability = FlightRenderPose["tailGeometry"];
export type WingDeformationAvailability = FlightRenderPose["wingDeformation"];
export type TailPresentationGeometry = Extract<TailPresentationGeometryAvailability, { kind: "available" }>["value"];
