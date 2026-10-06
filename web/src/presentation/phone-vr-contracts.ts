import type { StereoPresentationProfile } from "../render/contracts/runtime.js";
import type { PhoneGravityEvidence } from "./phone-vr-gravity.js";

export interface PhoneVrAvailability {
  readonly supported: boolean;
  readonly message: string;
}

export type PhoneVrPermissionResult =
  | { readonly ok: true }
  | { readonly ok: false; readonly message: string };

export interface PhoneVrSensorReading {
  readonly alpha: number | null;
  readonly beta: number | null;
  readonly gamma: number | null;
  readonly timestampMs: number;
  readonly gravityEvidence: PhoneGravityEvidence;
}

export interface PhoneVrSensorPort {
  checkAvailability(): Promise<PhoneVrAvailability>;
  requestPermissionFromUserGesture(): Promise<PhoneVrPermissionResult>;
  getScreenOrientationAngle(): number | null;
  startListening(
    onReading: (reading: PhoneVrSensorReading) => void,
    onScreenOrientationChange: (angle: number | null) => void
  ): void;
  stopListening(): void;
}

export interface PhoneVrGamepadConnection {
  readonly index: number;
  readonly generation: number;
}

export interface PhoneVrGamepadState {
  readonly connection: PhoneVrGamepadConnection;
  readonly axes: readonly number[];
  readonly buttons: readonly boolean[];
}

export interface PhoneVrGamepadInputPort {
  start(): void;
  stop(): void;
  readState(): PhoneVrGamepadState | null;
}

export const NO_PHONE_VR_GAMEPAD_INPUT: PhoneVrGamepadInputPort = Object.freeze({
  start: () => undefined,
  stop: () => undefined,
  readState: () => null
});

export interface PhoneVrOpticalProfile extends StereoPresentationProfile {
  readonly id: string;
  readonly version: 1;
  readonly validation: "unverified";
}

export const PHONE_VR_OPTICAL_PROFILE: PhoneVrOpticalProfile = Object.freeze({
  id: "generic-unverified-v1",
  version: 1,
  eyeSeparationMeters: 0.064,
  verticalFieldOfViewDegrees: 60,
  focusDistanceMeters: 10,
  distortion: "disabled",
  validation: "unverified"
});
