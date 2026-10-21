import {
  xr_chase_pose_json, xr_flight_relative_pose_json, xr_frd_pose_json,
  xr_pilot_eye_pose_json, xr_pose_chain_json, xr_reference_change_json,
  xr_registered_pilot_eye_json, xr_panel_camera_poses_json, xr_reference_frame_json, XrCameraBridge
} from "../../../pkg/birdman_game_wasm.js";
import type { XrCameraMount } from "../../generated/xr-camera-mount.js";
import type { XrCameraRig } from "../../generated/xr-camera-rig.js";
import type { XrFrdPose } from "../../generated/xr-frd-pose.js";
import type { XrPilotEyePose } from "../../generated/xr-pilot-eye-pose.js";
import type { XrPanelCameraPoses } from "../../generated/xr-panel-camera-poses.js";
import type { XrReferenceFrame } from "../../generated/xr-reference-frame.js";
import type { FlightRenderPose } from "../contracts/runtime.js";
import { IDENTITY_POSE } from "../contracts/math.js";
import type { Pose, Quaternion, Vec3 } from "../contracts/math.js";

export interface PilotEyePoint {
  readonly initialOffsetFrd: Vec3;
  readonly initialPilotPositionMeters: number;
  readonly orientationFrd: Quaternion;
}

export function registeredPilotEyePoint(): PilotEyePoint {
  const initial = readSpatial(() => xr_registered_pilot_eye_json()) as XrFrdPose;
  const physical = physicalPose(initial);
  return { initialOffsetFrd: physical.position, initialPilotPositionMeters: 0, orientationFrd: physical.orientation };
}

export function createPilotEyePoint(initialOffsetFrd: Vec3, initialPilotPositionMeters: number, orientationFrd: Quaternion): PilotEyePoint {
  const point = { initialOffsetFrd, initialPilotPositionMeters, orientationFrd };
  projectPilotEye(point, initialPilotPositionMeters, initialPilotPositionMeters);
  return point;
}

export function pilotEyePoseFrd(point: PilotEyePoint, position: number, initialPosition = point.initialPilotPositionMeters): Pose {
  return physicalPose(projectPilotEye(point, position, initialPosition).frd);
}

export function pilotEyePoseRender(point: PilotEyePoint, position: number, initialPosition = point.initialPilotPositionMeters): Pose {
  return projectPilotEye(point, position, initialPosition).render;
}

function projectPilotEye(point: PilotEyePoint, position: number, initialPosition: number): XrPilotEyePose {
  return readSpatial(() => xr_pilot_eye_pose_json(JSON.stringify(physicalObservation({ position: point.initialOffsetFrd, orientation: point.orientationFrd })), position, initialPosition)) as XrPilotEyePose;
}

export function poseFrdToRender(value: Pose): Pose {
  return readSpatial(() => xr_frd_pose_json(JSON.stringify(physicalObservation(value)))) as Pose;
}

export function chaseCameraPoseFrd(mode: "pilot" | "chase"): Pose {
  return mode === "pilot" ? IDENTITY_POSE : physicalPose(readSpatial(() => xr_chase_pose_json()) as XrFrdPose);
}

export function flightRelativePose(flight: FlightRenderPose, local: Pose): Pose {
  const datum = flight.datumPositionNed;
  return readSpatial(() => xr_flight_relative_pose_json(JSON.stringify([datum.north, datum.east, datum.down]), JSON.stringify(flight.attitudeBodyToNed), JSON.stringify(local))) as Pose;
}

export function projectXrPanelCameraPoses(rig: XrCameraRig, referenceFromPanel: Pose): XrPanelCameraPoses {
  return readSpatial(() => xr_panel_camera_poses_json(JSON.stringify(rig), JSON.stringify(referenceFromPanel))) as XrPanelCameraPoses;
}

export function projectXrReferenceFrame(rig: XrCameraRig, trackingFromHead: Pose): XrReferenceFrame {
  return readSpatial(() => xr_reference_frame_json(JSON.stringify(rig), JSON.stringify(trackingFromHead))) as XrReferenceFrame;
}

export class XrCameraOrigin {
  private bridge: XrCameraBridge | null = null;

  rig(aircraft: Pose, mount: XrCameraMount): XrCameraRig {
    this.bridge ??= new XrCameraBridge();
    return JSON.parse(this.bridge.camera_rig_json(JSON.stringify(aircraft), JSON.stringify(mount))) as XrCameraRig;
  }

  resetReference(previousFromNew: Pose): void {
    this.bridge ??= new XrCameraBridge();
    this.bridge.reset_reference_json(JSON.stringify(previousFromNew));
  }

  dispose(): void {
    const previous = this.bridge;
    this.bridge = null;
    previous?.free();
  }
}

export function composeXrPose(...poses: readonly Pose[]): Pose {
  return readSpatial(() => xr_pose_chain_json(JSON.stringify(poses))) as Pose;
}

export function xrReferenceChange(mount: Pose, newTrackingFromPrevious: Pose): Pose {
  return readSpatial(() => xr_reference_change_json(JSON.stringify(mount), JSON.stringify(newTrackingFromPrevious))) as Pose;
}

function physicalObservation(value: Pose): XrFrdPose {
  const point = value.position;
  const orientation = value.orientation;
  return { positionFrdM: [point.x, point.y, point.z], orientationFrdWxyz: [orientation.w, orientation.x, orientation.y, orientation.z] };
}

function physicalPose(value: XrFrdPose): Pose {
  const [x, y, z] = value.positionFrdM;
  const [w, qx, qy, qz] = value.orientationFrdWxyz;
  return { position: { x, y, z }, orientation: { w, x: qx, y: qy, z: qz } };
}

function readSpatial(operation: () => string): unknown {
  try { return JSON.parse(operation()) as unknown; }
  catch (cause) { throw new RangeError(String(cause), { cause }); }
}
