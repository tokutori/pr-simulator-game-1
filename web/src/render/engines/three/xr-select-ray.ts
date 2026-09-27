import { quaternion, rotateVec3, vec3 } from "../../contracts/math.js";
import type { SelectRay } from "../../contracts/runtime.js";

interface XrTargetPose {
  readonly transform: {
    readonly position: { readonly x: number; readonly y: number; readonly z: number };
    readonly orientation: { readonly x: number; readonly y: number; readonly z: number; readonly w: number };
  };
}

interface XrFrameLike<TargetSpace, ReferenceSpace> {
  readonly predictedDisplayTime: number;
  getPose(targetRaySpace: TargetSpace, referenceSpace: ReferenceSpace): XrTargetPose | null | undefined;
}

interface XrInputSourceLike<TargetSpace> {
  readonly targetRaySpace: TargetSpace;
}

export function selectRayFromXrEvent<TargetSpace, ReferenceSpace>(
  frame: XrFrameLike<TargetSpace, ReferenceSpace>,
  inputSource: XrInputSourceLike<TargetSpace>,
  referenceSpace: ReferenceSpace
): SelectRay | null {
  const targetPose = frame.getPose(inputSource.targetRaySpace, referenceSpace);
  if (targetPose === null || targetPose === undefined) return null;
  const transform = targetPose.transform;
  const orientation = quaternion(transform.orientation.w, transform.orientation.x, transform.orientation.y, transform.orientation.z);
  return Object.freeze({
    origin: vec3(transform.position.x, transform.position.y, transform.position.z),
    direction: rotateVec3(orientation, vec3(0, 0, -1)),
    timestampMs: frame.predictedDisplayTime
  });
}
