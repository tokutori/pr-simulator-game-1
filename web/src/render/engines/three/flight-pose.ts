import { Matrix4, Quaternion, Vector3 } from "three";
import { quaternion, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import type { FlightRenderPose } from "../../contracts/runtime.js";

const NED_TO_THREE = new Quaternion().setFromRotationMatrix(new Matrix4().makeBasis(
  new Vector3(0, 0, -1),
  new Vector3(1, 0, 0),
  new Vector3(0, -1, 0)
));

export function flightRelativePose(flight: FlightRenderPose, localPose: Pose): Pose {
  const bodyToNed = new Quaternion(
    flight.attitudeBodyToNed.x,
    flight.attitudeBodyToNed.y,
    flight.attitudeBodyToNed.z,
    flight.attitudeBodyToNed.w
  );
  const bodyToThree = NED_TO_THREE.clone();
  const worldFromLocal = NED_TO_THREE.clone().multiply(bodyToNed).multiply(bodyToThree.invert());
  const position = new Vector3(
    flight.datumPositionNed.east,
    -flight.datumPositionNed.down,
    -flight.datumPositionNed.north
  ).add(new Vector3(localPose.position.x, localPose.position.y, localPose.position.z).applyQuaternion(worldFromLocal));
  const orientation = worldFromLocal.multiply(new Quaternion(
    localPose.orientation.x,
    localPose.orientation.y,
    localPose.orientation.z,
    localPose.orientation.w
  ));
  return {
    position: vec3(position.x, position.y, position.z),
    orientation: quaternion(orientation.w, orientation.x, orientation.y, orientation.z)
  };
}
