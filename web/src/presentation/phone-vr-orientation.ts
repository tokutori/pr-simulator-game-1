import { multiplyQuaternion, quaternion, vec3 } from "../render/contracts/math.js";
import type { Quaternion } from "../render/contracts/math.js";
import type { PhoneVrSensorReading } from "./phone-vr-contracts.js";

const DEGREES_TO_RADIANS = Math.PI / 180;

export function phoneOrientationQuaternion(
  reading: PhoneVrSensorReading,
  screenOrientationAngle: number
): Quaternion | null {
  if (reading.alpha === null || reading.beta === null || reading.gamma === null ||
      !Number.isFinite(reading.alpha) || !Number.isFinite(reading.beta) ||
      !Number.isFinite(reading.gamma) || !Number.isFinite(screenOrientationAngle)) return null;
  const alphaRotation = axisRotation(vec3(0, 0, 1), reading.alpha * DEGREES_TO_RADIANS);
  const betaRotation = axisRotation(vec3(1, 0, 0), reading.beta * DEGREES_TO_RADIANS);
  const gammaRotation = axisRotation(vec3(0, 1, 0), reading.gamma * DEGREES_TO_RADIANS);
  const screenRotation = axisRotation(vec3(0, 0, 1), screenOrientationAngle * DEGREES_TO_RADIANS);
  return multiplyQuaternion(
    multiplyQuaternion(multiplyQuaternion(alphaRotation, betaRotation), gammaRotation),
    screenRotation
  );
}

function axisRotation(axis: { readonly x: number; readonly y: number; readonly z: number }, angle: number): Quaternion {
  const halfAngle = angle / 2;
  const sine = Math.sin(halfAngle);
  return quaternion(Math.cos(halfAngle), axis.x * sine, axis.y * sine, axis.z * sine);
}
