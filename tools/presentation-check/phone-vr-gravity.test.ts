import { describe, expect, it } from "vitest";
import { Matrix4, Quaternion as ThreeQuaternion, Vector3 } from "three";
import {
  calibratePhoneGravity,
  phoneGravityViewer,
  PHONE_GRAVITY_MINIMUM_HORIZONTAL_NORM,
  recenterPhoneGravity,
  referenceUpFromCoherentPhoneObservation
} from "../../web/src/presentation/phone-vr-gravity.js";
import type { PhoneGravityCalibration, PhoneGravityCalibrationInput, PhoneGravityEvidence } from "../../web/src/presentation/phone-vr-gravity.js";
import type { Quaternion, Vec3 } from "../../web/src/render/contracts/math.js";

const RADIANS = Math.PI / 180;
const SCREEN_ANGLES = [0, 90, -90, 180, 270];
const PITCHES = [-30, 0, 30];
const ROLLS = [-20, 0, 20];
const HEADINGS = [-170, -45, 0, 75, 180];
const EARTH: PhoneGravityEvidence = Object.freeze({ kind: "earth-z-up" });
const SOURCE_FROM_EARTH = new Matrix4().makeRotationZ(51 * RADIANS)
  .multiply(new Matrix4().makeRotationX(37 * RADIANS))
  .multiply(new Matrix4().makeRotationY(-23 * RADIANS));

describe("Phone gravity numerical calibration", () => {
  it.each(SCREEN_ANGLES)("preserves initial pitch and roll while setting only heading at screen %s", (screenAngleDegrees) => {
    for (const pitch of PITCHES) {
      for (const roll of ROLLS) {
        for (const heading of HEADINGS) {
          const result = calibrated(inputFor(headMatrix(heading, pitch, roll), screenAngleDegrees));
          expectRotation(result.viewer, headMatrix(0, pitch, roll));
          expectUpInHead(result.viewer, headMatrix(heading, pitch, roll));
          const vertical = phoneGravityViewer(result.calibration, deviceQuaternion(headMatrix(heading, 0, 0), screenAngleDegrees), screenAngleDegrees);
          expect(vertical.kind).toBe("oriented");
          if (vertical.kind !== "oriented") throw new Error("Expected a finite viewer");
          expectRotation(vertical.viewer, new Matrix4());
          expect(new Vector3(0, 0, -1).applyQuaternion(toThree(vertical.viewer)).y).toBeCloseTo(0, 12);
        }
      }
    }
  });

  it.each(SCREEN_ANGLES)("is invariant under arbitrary relative reference rotations at screen %s", (screenAngleDegrees) => {
    const references = [SOURCE_FROM_EARTH, new Matrix4().makeRotationX(Math.PI), new Matrix4().makeRotationY(Math.PI), new Matrix4().makeRotationZ(Math.PI)];
    for (const referenceFromEarth of references) {
      for (const pitch of PITCHES) {
        for (const roll of ROLLS) {
          for (const heading of HEADINGS) {
            const head = headMatrix(heading, pitch, roll);
            const input = inputFor(head, screenAngleDegrees, referenceFromEarth);
            const result = calibrated(input);
            expectRotation(result.viewer, headMatrix(0, pitch, roll));
            const referenceUp = new Vector3(0, 0, 1).applyMatrix4(referenceFromEarth);
            expectVector(referenceUp.applyQuaternion(toThree(result.calibration.trackingFromReference)), new Vector3(0, 1, 0));
          }
        }
      }
    }
  });

  it("retains a fixed basis through zenith and nadir without extracting a new heading", () => {
    const start = calibrated(inputFor(headMatrix(40, 30, 20), 90, SOURCE_FROM_EARTH));
    const original = structuredClone(start.calibration);
    for (const pitch of [60, 89.9, 90, 90.1, 120, -60, -89.9, -90, -90.1, -120]) {
      const result = phoneGravityViewer(start.calibration, deviceQuaternion(headMatrix(40, pitch, 20), 90, SOURCE_FROM_EARTH), 90);
      expect(result.kind).toBe("oriented");
      if (result.kind !== "oriented") throw new Error("Active orientation must remain defined at the poles");
      expectRotation(result.viewer, headMatrix(0, pitch, 20));
    }
    expect(start.calibration).toEqual(original);
  });

  it.each(SCREEN_ANGLES)("keeps one calibration across paired device and screen orientation changes to %s", (screenAngleDegrees) => {
    const start = calibrated(inputFor(headMatrix(50, -30, 20), 0, SOURCE_FROM_EARTH));
    const result = phoneGravityViewer(start.calibration, deviceQuaternion(headMatrix(65, 30, -20), screenAngleDegrees, SOURCE_FROM_EARTH), screenAngleDegrees);
    expect(result.kind).toBe("oriented");
    if (result.kind !== "oriented") throw new Error("Expected a paired screen/device update");
    expectRotation(result.viewer, headMatrix(15, 30, -20));
  });

  it.each(SCREEN_ANGLES)("manual recenter applies only a new-from-old tracking yaw at screen %s", (screenAngleDegrees) => {
    const initial = calibrated(inputFor(headMatrix(-40, -30, -20), screenAngleDegrees, SOURCE_FROM_EARTH));
    const nextDevice = deviceQuaternion(headMatrix(75, 30, 20), screenAngleDegrees, SOURCE_FROM_EARTH);
    const before = phoneGravityViewer(initial.calibration, nextDevice, screenAngleDegrees);
    expect(before.kind).toBe("oriented");
    if (before.kind !== "oriented") throw new Error("Expected a finite pre-recenter viewer");
    const result = recenterPhoneGravity(initial.calibration, nextDevice, screenAngleDegrees);
    expect(result.kind).toBe("recentered");
    if (result.kind !== "recentered") throw new Error("Expected nondegenerate recenter");
    expectRotation(result.viewer, headMatrix(0, 30, 20));
    expectRotation(result.newTrackingFromOldTracking, new Matrix4().makeRotationY(-115 * RADIANS));
    expectVector(new Vector3(0, 1, 0).applyQuaternion(toThree(result.newTrackingFromOldTracking)), new Vector3(0, 1, 0));
    expectUpInHead(result.viewer, new Matrix4().makeRotationFromQuaternion(toThree(before.viewer)));
    const mountedTracking = new Matrix4().makeTranslation(0.3, 1.1, -0.2)
      .multiply(new Matrix4().makeRotationZ(0.3));
    const oldHead = mountedTracking.clone().multiply(new Matrix4().makeRotationFromQuaternion(toThree(before.viewer)));
    const oldPanel = new Matrix4().makeTranslation(0.2, 0.4, -2.4);
    const delta = new Matrix4().makeRotationFromQuaternion(toThree(result.newTrackingFromOldTracking));
    const newPanel = mountedTracking.clone().multiply(delta).multiply(mountedTracking.clone().invert()).multiply(oldPanel);
    const newHead = mountedTracking.clone().multiply(new Matrix4().makeRotationFromQuaternion(toThree(result.viewer)));
    expectMatrix(newHead.invert().multiply(newPanel), oldHead.invert().multiply(oldPanel));
    const later = phoneGravityViewer(result.calibration, deviceQuaternion(headMatrix(90, -30, -20), screenAngleDegrees, SOURCE_FROM_EARTH), screenAngleDegrees);
    expect(later.kind).toBe("oriented");
    if (later.kind !== "oriented") throw new Error("Expected updated calibration to remain usable");
    expectRotation(later.viewer, headMatrix(15, -30, -20));
    expect(initial.calibration).not.toBe(result.calibration);
  });

  it("defines the one-degree pole cone as an engineering conditioning threshold", () => {
    expect(PHONE_GRAVITY_MINIMUM_HORIZONTAL_NORM).toBe(Math.sin(Math.PI / 180));
    for (const sign of [-1, 1]) {
      for (const pitch of [sign * 90, sign * (89 + 1e-6)]) {
        expect(calibratePhoneGravity(inputFor(headMatrix(70, pitch, 20), 0))).toEqual({ kind: "pending", reason: "heading-degenerate" });
      }
      const outside = calibrated(inputFor(headMatrix(70, sign * (89 - 1e-6), 20), 0));
      expectRotation(outside.viewer, headMatrix(0, sign * (89 - 1e-6), 20));
    }
  });

  it("retains the previous calibration on a degenerate manual recenter and permits a later retry", () => {
    const initial = calibrated(inputFor(headMatrix(20, 0, 0), -90));
    const saved = structuredClone(initial.calibration);
    for (const pitch of [90, -90, 89.5, -89.5]) {
      const result = recenterPhoneGravity(initial.calibration, deviceQuaternion(headMatrix(65, pitch, 20), -90), -90);
      expect(result.kind).toBe("retained");
      if (result.kind !== "retained") throw new Error("Expected retained calibration");
      expect(result.reason).toBe("heading-degenerate");
      expect(result.calibration).toBe(initial.calibration);
      expectRotation(result.viewer, headMatrix(45, pitch, 20));
    }
    expect(initial.calibration).toEqual(saved);
    const retry = recenterPhoneGravity(initial.calibration, deviceQuaternion(headMatrix(65, 30, 20), -90), -90);
    expect(retry.kind).toBe("recentered");
    if (retry.kind !== "recentered") throw new Error("Expected valid manual retry");
    expectRotation(retry.viewer, headMatrix(0, 30, 20));
  });

  it("returns pending for missing gravity without manufacturing a default calibration", () => {
    const result = calibratePhoneGravity({ ...inputFor(headMatrix(0, 0, 0), 0), evidence: { kind: "unavailable" } });
    expect(result).toEqual({ kind: "pending", reason: "missing-gravity" });
    expect(result).not.toHaveProperty("calibration");
    expect(result).not.toHaveProperty("viewer");
  });

  it("maps a caller-established coherent device-up observation into its arbitrary reference", () => {
    const input = inputFor(headMatrix(65, 30, -20), 90, SOURCE_FROM_EARTH);
    const referenceUp = new Vector3(0, 0, 1).applyMatrix4(SOURCE_FROM_EARTH);
    const deviceUp = referenceUp.clone().applyQuaternion(toThree(input.referenceFromDevice).invert()).multiplyScalar(9.8);
    const result = referenceUpFromCoherentPhoneObservation({ referenceFromDeviceAtObservation: input.referenceFromDevice, deviceUpAtObservation: deviceUp });
    expect(result.kind).toBe("derived");
    if (result.kind !== "derived") throw new Error("Expected a reference-space up direction");
    expectVector(new Vector3().copy(result.evidence.referenceUp), referenceUp);
    expectRotation(calibrated({ ...input, evidence: result.evidence }).viewer, headMatrix(0, 30, -20));
  });

  it.each([Number.MIN_VALUE, 9.8, Number.MAX_VALUE])("normalizes positive gravity magnitude %s without changing its direction", (magnitude) => {
    const result = referenceUpFromCoherentPhoneObservation({
      referenceFromDeviceAtObservation: { w: 1, x: 0, y: 0, z: 0 },
      deviceUpAtObservation: { x: 0, y: 0, z: magnitude }
    });
    expect(result).toEqual({ kind: "derived", evidence: { kind: "relative-reference-up", referenceUp: { x: 0, y: 0, z: 1 } } });
  });

  it("preserves the sign of a coherent up observation instead of guessing the closer hemisphere", () => {
    const result = referenceUpFromCoherentPhoneObservation({
      referenceFromDeviceAtObservation: { w: 1, x: 0, y: 0, z: 0 },
      deviceUpAtObservation: { x: 0, y: 0, z: -9.8 }
    });
    expect(result).toEqual({ kind: "derived", evidence: { kind: "relative-reference-up", referenceUp: { x: 0, y: 0, z: -1 } } });
  });

  it.each([NaN, Infinity, -Infinity])("rejects nonfinite numerical input %s at every public entry", (invalidValue) => {
    const initialInput = inputFor(headMatrix(20, 30, 20), 0);
    const initial = calibrated(initialInput);
    const badRotation = { ...initialInput.referenceFromDevice, w: invalidValue };
    const expected = { kind: "invalid", reason: "nonfinite" };
    expect(calibratePhoneGravity({ ...initialInput, referenceFromDevice: badRotation })).toEqual(expected);
    expect(calibratePhoneGravity({ ...initialInput, screenAngleDegrees: invalidValue })).toEqual(expected);
    expect(calibratePhoneGravity({ ...initialInput, evidence: { kind: "relative-reference-up", referenceUp: { x: invalidValue, y: 0, z: 1 } } })).toEqual(expected);
    expect(phoneGravityViewer(initial.calibration, badRotation, 0)).toEqual(expected);
    expect(phoneGravityViewer(initial.calibration, initialInput.referenceFromDevice, invalidValue)).toEqual(expected);
    expect(recenterPhoneGravity(initial.calibration, badRotation, 0)).toEqual(expected);
    expect(recenterPhoneGravity(initial.calibration, initialInput.referenceFromDevice, invalidValue)).toEqual(expected);
    expect(referenceUpFromCoherentPhoneObservation({ referenceFromDeviceAtObservation: badRotation, deviceUpAtObservation: { x: 0, y: 0, z: 1 } })).toEqual(expected);
    expect(referenceUpFromCoherentPhoneObservation({ referenceFromDeviceAtObservation: initialInput.referenceFromDevice, deviceUpAtObservation: { x: 0, y: invalidValue, z: 1 } })).toEqual(expected);
  });

  it.each([0, 2, Number.MAX_VALUE])("rejects invalid rotation magnitude %s without silently normalizing it", (magnitude) => {
    const input = inputFor(headMatrix(0, 0, 0), 0);
    const badRotation = { w: magnitude, x: 0, y: 0, z: 0 };
    const expected = { kind: "invalid", reason: "invalid-rotation" };
    expect(calibratePhoneGravity({ ...input, referenceFromDevice: badRotation })).toEqual(expected);
    expect(phoneGravityViewer({ trackingFromReference: badRotation }, input.referenceFromDevice, 0)).toEqual(expected);
    expect(recenterPhoneGravity({ trackingFromReference: badRotation }, input.referenceFromDevice, 0)).toEqual(expected);
    expect(referenceUpFromCoherentPhoneObservation({ referenceFromDeviceAtObservation: badRotation, deviceUpAtObservation: { x: 0, y: 0, z: 1 } })).toEqual(expected);
  });

  it("rejects zero up without conflating invalid data with missing evidence", () => {
    const input = inputFor(headMatrix(0, 0, 0), 0);
    const zero = { x: 0, y: 0, z: 0 };
    expect(calibratePhoneGravity({ ...input, evidence: { kind: "relative-reference-up", referenceUp: zero } })).toEqual({ kind: "invalid", reason: "invalid-up" });
    expect(referenceUpFromCoherentPhoneObservation({ referenceFromDeviceAtObservation: input.referenceFromDevice, deviceUpAtObservation: zero })).toEqual({ kind: "invalid", reason: "invalid-up" });
  });

  it("treats opposite quaternion signs as the same rotation", () => {
    const input = inputFor(headMatrix(180, -30, 20), 270, SOURCE_FROM_EARTH);
    const rotation = input.referenceFromDevice;
    const negated = { w: -rotation.w, x: -rotation.x, y: -rotation.y, z: -rotation.z };
    expectRotation(calibrated({ ...input, referenceFromDevice: negated }).viewer, headMatrix(0, -30, 20));
  });

  it("produces immutable deterministic values and leaves input and previous calibration unchanged", () => {
    const input = inputFor(headMatrix(75, -30, 20), -90, SOURCE_FROM_EARTH);
    const before = structuredClone(input);
    const first = calibrated(input);
    const second = calibrated(input);
    expect(first).toEqual(second);
    expect(Object.isFrozen(first)).toBe(true);
    expect(Object.isFrozen(first.calibration)).toBe(true);
    expect(Object.isFrozen(first.calibration.trackingFromReference)).toBe(true);
    expect(Object.isFrozen(first.viewer)).toBe(true);
    expect(input).toEqual(before);
    const saved = structuredClone(first.calibration);
    const next = recenterPhoneGravity(first.calibration, deviceQuaternion(headMatrix(90, 30, 0), -90, SOURCE_FROM_EARTH), -90);
    expect(next.kind).toBe("recentered");
    expect(first.calibration).toEqual(saved);
  });
});

function inputFor(head: Matrix4, screenAngleDegrees: number, referenceFromEarth?: Matrix4): PhoneGravityCalibrationInput {
  return Object.freeze({
    referenceFromDevice: deviceQuaternion(head, screenAngleDegrees, referenceFromEarth),
    screenAngleDegrees,
    evidence: referenceFromEarth === undefined ? EARTH : Object.freeze({
      kind: "relative-reference-up",
      referenceUp: Object.freeze(new Vector3(0, 0, 1).applyMatrix4(referenceFromEarth))
    })
  });
}

function deviceQuaternion(head: Matrix4, screenAngleDegrees: number, referenceFromEarth = new Matrix4()): Quaternion {
  const matrix = referenceFromEarth.clone().multiply(new Matrix4().makeRotationX(Math.PI / 2))
    .multiply(head).multiply(new Matrix4().makeRotationZ(screenAngleDegrees * RADIANS));
  const rotation = new ThreeQuaternion().setFromRotationMatrix(matrix);
  return Object.freeze({ w: rotation.w, x: rotation.x, y: rotation.y, z: rotation.z });
}

function headMatrix(heading: number, pitch: number, roll: number): Matrix4 {
  return new Matrix4().makeRotationY(heading * RADIANS)
    .multiply(new Matrix4().makeRotationX(pitch * RADIANS))
    .multiply(new Matrix4().makeRotationZ(roll * RADIANS));
}

function calibrated(input: PhoneGravityCalibrationInput): { readonly calibration: PhoneGravityCalibration; readonly viewer: Quaternion } {
  const result = calibratePhoneGravity(input);
  expect(result.kind).toBe("calibrated");
  if (result.kind !== "calibrated") throw new Error(`Unexpected calibration result: ${result.kind}`);
  return result;
}

function toThree(value: Quaternion): ThreeQuaternion {
  return new ThreeQuaternion(value.x, value.y, value.z, value.w);
}

function expectRotation(actual: Quaternion, expected: Matrix4): void {
  const actualMatrix = new Matrix4().makeRotationFromQuaternion(toThree(actual));
  expectMatrix(actualMatrix, expected);
  expect(Math.hypot(actual.w, actual.x, actual.y, actual.z)).toBeCloseTo(1, 12);
}

function expectUpInHead(actual: Quaternion, expectedHead: Matrix4): void {
  expectVector(
    new Vector3(0, 1, 0).applyQuaternion(toThree(actual).invert()),
    new Vector3(0, 1, 0).transformDirection(expectedHead.clone().invert())
  );
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  for (let index = 0; index < 16; index += 1) expect(actual.elements[index]).toBeCloseTo(expected.elements[index] ?? NaN, 10);
}

function expectVector(actual: Vec3, expected: Vec3): void {
  expect(actual.x).toBeCloseTo(expected.x, 10);
  expect(actual.y).toBeCloseTo(expected.y, 10);
  expect(actual.z).toBeCloseTo(expected.z, 10);
}
