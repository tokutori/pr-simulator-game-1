/** The measured launch point is the origin of the local NED flight frame. */
export const LAUNCH_ORIGIN_WGS84 = Object.freeze({
  latitudeDegrees: 35.294075,
  longitudeDegrees: 136.254448
});

/** Approximate platform dimensions; the front lip is at the NED origin. */
export const LAUNCH_PLATFORM = Object.freeze({
  widthMeters: 12,
  lengthMeters: 20,
  frontLipAboveWaterMeters: 10,
  downwardSlopeDegrees: 3.5,
  launchBearingDegrees: 315
});

const metersPerLatitudeDegree = 111_132;
const metersPerLongitudeDegree = 111_320 * Math.cos(LAUNCH_ORIGIN_WGS84.latitudeDegrees * Math.PI / 180);

/** Local tangent-plane conversion for the short game course, not a surveying transform. */
export function nedToWgs84(northMeters: number, eastMeters: number): Readonly<{ latitudeDegrees: number; longitudeDegrees: number }> {
  return {
    latitudeDegrees: LAUNCH_ORIGIN_WGS84.latitudeDegrees + northMeters / metersPerLatitudeDegree,
    longitudeDegrees: LAUNCH_ORIGIN_WGS84.longitudeDegrees + eastMeters / metersPerLongitudeDegree
  };
}
