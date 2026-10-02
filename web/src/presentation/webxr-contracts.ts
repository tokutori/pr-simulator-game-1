import type { Pose } from "../render/contracts/math.js";

export interface WebXrAvailability {
  readonly supported: boolean;
  readonly message: string;
}

export type WebXrSessionRequest =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string };

export interface WebXrSessionPort {
  transformTrackingPose(pose: Pose): Pose;
  checkAvailability(): Promise<WebXrAvailability>;
  requestSessionFromUserGesture(): Promise<WebXrSessionRequest>;
  startSession(): Promise<void>;
  endSession(): Promise<void>;
  setSessionEndHandler(handler: (() => void) | null): void;
  setReferenceSpaceResetHandler(handler: ((previousReferenceFromNew: Pose | null) => void) | null): void;
}
