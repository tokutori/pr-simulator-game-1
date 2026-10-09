import { describe, expect, it } from "vitest";
import {
  createPilotEyePoint, pilotEyePoseThree, poseFrdToThree, SYNTHETIC_PILOT_EYE_POINT
} from "../../web/src/render/camera/pilot-eye-point.js";
import { replayCameraPoseFrd } from "../../web/src/render/camera/replay-camera.js";
import {
  IDENTITY_POSE, multiplyQuaternion, quaternion, rotateVec3, transformPoint, vec3
} from "../../web/src/render/contracts/math.js";
import type { Quaternion, Vec3 } from "../../web/src/render/contracts/math.js";
import { flightRelativePose } from "../../web/src/render/engines/three/flight-pose.js";

describe("FRD camera basis", () => {
  it.each([
    ["positive roll", Math.PI / 2, 0, 0],
    ["negative roll", -Math.PI / 2, 0, 0],
    ["positive pitch", 0, Math.PI / 2, 0],
    ["negative pitch", 0, -Math.PI / 2, 0],
    ["positive yaw", 0, 0, Math.PI / 2],
    ["negative yaw", 0, 0, -Math.PI / 2],
    ["combined launch attitude", 0.37, -0.28, -Math.PI / 4],
    ["combined reverse attitude", -0.46, 0.31, 2.1]
  ] as const)("maps %s using the same axes as position", (_label, roll, pitch, yaw) => {
    const eyePoint = createPilotEyePoint(vec3(1.2, 0.04, -0.08), 0.35, frdEulerQuaternion(roll, pitch, yaw));
    const eye = pilotEyePoseThree(eyePoint, 0.65);
    const eyePositionFrd = vec3(1.5, 0.04, -0.08);
    expectVector(eye.position, rendererVector(eyePositionFrd));

    for (const localVectorFrd of [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1), vec3(0.7, -0.2, 0.4)]) {
      const rotatedFrd = rotateFrdEuler(localVectorFrd, roll, pitch, yaw);
      expectVector(rotateVec3(eye.orientation, rendererVector(localVectorFrd)), rendererVector(rotatedFrd));
      expectVector(transformPoint(eye, rendererVector(localVectorFrd)), rendererVector(vec3(
        eyePositionFrd.x + rotatedFrd.x,
        eyePositionFrd.y + rotatedFrd.y,
        eyePositionFrd.z + rotatedFrd.z
      )));
    }
  });

  it("preserves the identity Pilot mount and forward view", () => {
    const eye = pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0, 0);
    expectVector(eye.position, vec3(0, 0.15, -0.55));
    expect(eye.orientation).toEqual(IDENTITY_POSE.orientation);
    expectVector(rotateVec3(eye.orientation, vec3(0, 0, -1)), vec3(0, 0, -1));
    expectVector(rotateVec3(eye.orientation, vec3(0, 1, 0)), vec3(0, 1, 0));
    const pilot = poseFrdToThree(replayCameraPoseFrd("pilot"));
    expectVector(pilot.position, IDENTITY_POSE.position);
    expect(pilot.orientation).toEqual(IDENTITY_POSE.orientation);
  });

  it("keeps Pilot forward aligned with the northwest launch heading", () => {
    const heading = -Math.PI / 4;
    const eye = flightRelativePose({
      datumPositionNed: { north: 0, east: 0, down: -10.5 },
      attitudeBodyToNed: frdEulerQuaternion(0, 0, heading),
      pilotPositionMeters: 0,
      initialPilotPositionMeters: 0
    }, pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0));

    expectVector(rotateVec3(eye.orientation, vec3(0, 0, -1)), vec3(Math.sin(heading), 0, -Math.cos(heading)));
    expectVector(eye.position, vec3(0.55 * Math.sin(heading), 10.65, -0.55 * Math.cos(heading)));
  });

  it("aims the converted Chase pose down toward the aircraft datum", () => {
    const chase = poseFrdToThree(replayCameraPoseFrd("chase"));
    expectVector(chase.position, vec3(0, 4, 12));
    expectVector(rotateVec3(chase.orientation, vec3(0, 0, -1)), unitVector(vec3(0, -4, -12)));
  });

  it.each([
    ["identity", 0, 0, 0],
    ["northwest launch", 0, 0, -Math.PI / 4],
    ["combined aircraft attitude", 0.25, -0.18, 0.45]
  ] as const)("keeps Chase aimed at the aircraft after %s", (_label, roll, pitch, yaw) => {
    const datumPositionNed = { north: 21, east: -7, down: -10.5 };
    const chase = flightRelativePose({
      datumPositionNed,
      attitudeBodyToNed: frdEulerQuaternion(roll, pitch, yaw),
      pilotPositionMeters: 0,
      initialPilotPositionMeters: 0
    }, poseFrdToThree(replayCameraPoseFrd("chase")));
    const datum = rendererVector(vec3(datumPositionNed.north, datumPositionNed.east, datumPositionNed.down));
    const offset = rendererVector(rotateFrdEuler(vec3(-12, 0, -4), roll, pitch, yaw));
    expectVector(chase.position, vec3(datum.x + offset.x, datum.y + offset.y, datum.z + offset.z));
    expectVector(rotateVec3(chase.orientation, vec3(0, 0, -1)), unitVector(vec3(
      datum.x - chase.position.x,
      datum.y - chase.position.y,
      datum.z - chase.position.z
    )));
  });
});

function rendererVector(valueFrd: Vec3): Vec3 {
  return vec3(valueFrd.y, -valueFrd.z, -valueFrd.x);
}

function frdEulerQuaternion(roll: number, pitch: number, yaw: number): Quaternion {
  const rollRotation = quaternion(Math.cos(roll / 2), Math.sin(roll / 2), 0, 0);
  const pitchRotation = quaternion(Math.cos(pitch / 2), 0, Math.sin(pitch / 2), 0);
  const yawRotation = quaternion(Math.cos(yaw / 2), 0, 0, Math.sin(yaw / 2));
  return multiplyQuaternion(multiplyQuaternion(yawRotation, pitchRotation), rollRotation);
}

function rotateFrdEuler(value: Vec3, roll: number, pitch: number, yaw: number): Vec3 {
  const rolled = vec3(value.x,
    Math.cos(roll) * value.y - Math.sin(roll) * value.z,
    Math.sin(roll) * value.y + Math.cos(roll) * value.z);
  const pitched = vec3(Math.cos(pitch) * rolled.x + Math.sin(pitch) * rolled.z,
    rolled.y, -Math.sin(pitch) * rolled.x + Math.cos(pitch) * rolled.z);
  return vec3(Math.cos(yaw) * pitched.x - Math.sin(yaw) * pitched.y,
    Math.sin(yaw) * pitched.x + Math.cos(yaw) * pitched.y, pitched.z);
}

function unitVector(value: Vec3): Vec3 {
  const magnitude = Math.hypot(value.x, value.y, value.z);
  return vec3(value.x / magnitude, value.y / magnitude, value.z / magnitude);
}

function expectVector(actual: Vec3, expected: Vec3): void {
  expect(actual.x).toBeCloseTo(expected.x, 12);
  expect(actual.y).toBeCloseTo(expected.y, 12);
  expect(actual.z).toBeCloseTo(expected.z, 12);
}
