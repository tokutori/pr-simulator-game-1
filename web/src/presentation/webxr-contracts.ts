import type { Pose } from "../render/contracts/math.js";
import type { XrReferenceFrame } from "../generated/xr-reference-frame.js";

export interface WebXrAvailability {
  readonly supported: boolean;
  readonly message: string;
}

export type WebXrSessionRequest =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string };

export interface WebXrSessionPort {
  projectTrackingFrame(pose: Pose): XrReferenceFrame;
  checkAvailability(): Promise<WebXrAvailability>;
  requestSessionFromUserGesture(): Promise<WebXrSessionRequest>;
  startSession(): Promise<void>;
  endSession(): Promise<void>;
  setSessionEndHandler(handler: (() => void) | null): void;
  setReferenceSpaceResetHandler(handler: ((previousReferenceFromNew: Pose | null) => void) | null): void;
}
