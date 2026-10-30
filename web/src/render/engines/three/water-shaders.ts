const LAKE_WAVE_PACKET_GLSL = /* glsl */ `
float lakeWaveHash(vec2 cell) {
  return fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
}

vec3 lakeValueNoise(vec2 point) {
  vec2 cell = floor(point);
  vec2 fraction = fract(point);
  vec2 blend = fraction * fraction * (3.0 - 2.0 * fraction);
  vec2 blendGradient = 6.0 * fraction * (1.0 - fraction);
  float lowerLeft = lakeWaveHash(cell);
  float lowerRight = lakeWaveHash(cell + vec2(1.0, 0.0));
  float upperLeft = lakeWaveHash(cell + vec2(0.0, 1.0));
  float upperRight = lakeWaveHash(cell + vec2(1.0, 1.0));
  float value = mix(mix(lowerLeft, lowerRight, blend.x), mix(upperLeft, upperRight, blend.x), blend.y);
  vec2 gradient = vec2(
    blendGradient.x * mix(lowerRight - lowerLeft, upperRight - upperLeft, blend.y),
    blendGradient.y * mix(upperLeft - lowerLeft, upperRight - lowerRight, blend.x)
  );
  return vec3(value * 2.0 - 1.0, gradient * 2.0);
}

mat3 lakeWaveModulation(vec2 point, vec2 direction, float waveNumber, float seed) {
  vec2 across = vec2(-direction.y, direction.x);
  float frequency = max(0.11, waveNumber * 0.055);
  vec2 coordinate = vec2(dot(point, across), dot(point, direction)) * frequency + vec2(seed * 1.7, seed * 2.3);
  vec2 secondCoordinate = vec2(coordinate.x * 1.7 + coordinate.y * 0.37,
    coordinate.y * 1.9 - coordinate.x * 0.24) + vec2(13.7, -8.2);
  vec3 broad = lakeValueNoise(coordinate);
  vec3 fine = lakeValueNoise(secondCoordinate);
  vec2 broadGradient = frequency * (across * broad.y + direction * broad.z);
  vec2 fineGradient = frequency * (fine.y * (across * 1.7 + direction * 0.37)
    + fine.z * (across * -0.24 + direction * 1.9));
  float packet = 1.0 + 0.38 * broad.x + 0.12 * fine.x;
  vec2 packetGradient = 0.38 * broadGradient + 0.12 * fineGradient;
  float offset = 4.6 * broad.x + 1.4 * fine.x;
  vec2 offsetGradient = 4.6 * broadGradient + 1.4 * fineGradient;
  return mat3(vec3(packet, packetGradient), vec3(offset, offsetGradient), vec3(0.0));
}
`;

const LAKE_WAVE_PROJECTION_GLSL = /* glsl */ `
#if LAKE_WAVE_COUNT > 0
uniform vec4 uWaveKAmplitude[LAKE_WAVE_COUNT];
uniform vec4 uWaveOmegaPhase[LAKE_WAVE_COUNT];
#endif
${LAKE_WAVE_PACKET_GLSL}

struct LakeWaveProjection {
  vec3 position;
  vec4 horizontalJacobian;
  vec2 heightGradient;
  float crest;
  float slopeEnergy;
};

vec2 lakeGridSpacingGradient(vec2 localPoint, float magnitude) {
  vec2 absolutePoint = abs(localPoint);
  if (absolutePoint.x > absolutePoint.y) return vec2(sign(localPoint.x) * magnitude, 0.0);
  if (absolutePoint.y > absolutePoint.x) return vec2(0.0, sign(localPoint.y) * magnitude);
  return sign(localPoint) * (0.5 * magnitude);
}

LakeWaveProjection projectLakeWaves(vec3 localPosition, vec2 worldXZ, float gridSpacing,
    float spacingGradientMagnitude, float timeSeconds, float choppiness) {
  LakeWaveProjection result;
  result.position = localPosition;
  result.horizontalJacobian = vec4(1.0, 0.0, 0.0, 1.0);
  result.heightGradient = vec2(0.0);
  result.crest = 0.0;
  result.slopeEnergy = 0.0;
  vec2 spacingGradient = lakeGridSpacingGradient(localPosition.xz, spacingGradientMagnitude);
  #if LAKE_WAVE_COUNT > 0
  #pragma unroll_loop_start
  for (int i = 0; i < LAKE_WAVE_LOOP_COUNT; i++) {
    {
      vec4 wave = uWaveKAmplitude[i];
      vec4 timing = uWaveOmegaPhase[i];
      vec2 direction = wave.xy;
      float waveNumber = wave.z;
      float visibility = 1.0 - smoothstep(1.3, 2.5, waveNumber * gridSpacing);
      float transition = clamp((waveNumber * gridSpacing - 1.3) / 1.2, 0.0, 1.0);
      vec2 visibilityGradient = (-6.0 * waveNumber / 1.2 * transition * (1.0 - transition)) * spacingGradient;
      result.slopeEnergy += waveNumber * wave.w;
      if (!(visibility == 0.0 && all(equal(visibilityGradient, vec2(0.0))))) {
        mat3 modulation = lakeWaveModulation(worldXZ, direction, waveNumber, timing.y);
        vec3 packet = modulation[0];
        vec3 warp = modulation[1];
        float amplitude = wave.w * visibility * packet.x;
        vec2 amplitudeGradient = wave.w * (visibility * packet.yz + packet.x * visibilityGradient);
        float phase = waveNumber * dot(direction, worldXZ) + warp.x - timing.x * timeSeconds + timing.y;
        vec2 phaseGradient = waveNumber * direction + warp.yz;
        float phaseSine = sin(phase);
        float phaseCosine = cos(phase);
        float crestBasis = 0.5 + 0.5 * phaseSine;
        result.position.xz += choppiness * amplitude * direction * phaseCosine;
        result.position.y += amplitude * (2.0 * crestBasis * crestBasis * crestBasis - 0.625);
        vec2 horizontalGradient = choppiness * (phaseCosine * amplitudeGradient - amplitude * phaseSine * phaseGradient);
        result.horizontalJacobian += vec4(direction.x * horizontalGradient, direction.y * horizontalGradient);
        result.heightGradient += amplitude * 3.0 * crestBasis * crestBasis * phaseCosine * phaseGradient
          + (2.0 * crestBasis * crestBasis * crestBasis - 0.625) * amplitudeGradient;
        result.crest += phaseSine * waveNumber * amplitude;
      }
    }
  }
  #pragma unroll_loop_end
  #endif
  return result;
}
`;

const lakeWaterVertexShader = /* glsl */ `
uniform float uTimeSeconds;
uniform vec4 uWindVelocity;
uniform float uDetailScale;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform vec4 uDetailExtents;
uniform float uChoppiness;
uniform float uGridSpacingGradient;
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <shadowmap_pars_vertex>
${LAKE_WAVE_PROJECTION_GLSL}

void main() {
  vec3 p = position;
  vec2 waveXZ = (modelMatrix * vec4(p, 1.0)).xz;
  vSurfaceGradient.xy = waveXZ;
  vec2 detailDrift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearDetail = texture2D(uDetailNear, (waveXZ - detailDrift * 0.42) / uDetailExtents.x);
  vec4 farDetail = texture2D(uDetailFar, (waveXZ - detailDrift * 0.89) / uDetailExtents.y);
  float nearHeight = nearDetail.a - 128.0 / 255.0;
  float farHeight = farDetail.a - 128.0 / 255.0;
  float nearVisibility = 1.0 - smoothstep(0.12, 0.32, aGridSpacing);
  float farVisibility = 1.0 - smoothstep(0.8, 2.8, aGridSpacing);
  p.y += (nearHeight * 0.6 * nearVisibility + farHeight * 0.55 * farVisibility) * uDetailScale;
  vec2 nearSlope = (nearDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 farSlope = (farDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 resolvedDetailSlope = (nearSlope * (0.6 * nearVisibility)
    + farSlope * (0.55 * farVisibility)) * uDetailScale;
  LakeWaveProjection projected = projectLakeWaves(p, waveXZ, aGridSpacing,
    uGridSpacingGradient, uTimeSeconds, uChoppiness);
  p = projected.position;
  float dxx = projected.horizontalJacobian.x;
  float dxz = projected.horizontalJacobian.y;
  float dzx = projected.horizontalJacobian.z;
  float dzz = projected.horizontalJacobian.w;
  vec2 heightGradient = projected.heightGradient + resolvedDetailSlope;
  float horizontalDeterminant = dxx * dzz - dxz * dzx;
  // The geometric normal is measured after Gerstner horizontal displacement.
  // Convert the represented detail slope into that same coordinate system.
  vSurfaceGradient.zw = vec2(
    dzz * resolvedDetailSlope.x - dzx * resolvedDetailSlope.y,
    -dxz * resolvedDetailSlope.x + dxx * resolvedDetailSlope.y
  ) / max(horizontalDeterminant, 0.01);
  vec3 tangentX = vec3(dxx, heightGradient.x, dzx);
  vec3 tangentZ = vec3(dxz, heightGradient.y, dzz);
  vec3 localNormal = normalize(cross(tangentZ, tangentX));
  vec4 worldPosition = modelMatrix * vec4(p, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * localNormal);
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    vDirectionalShadowCoord[0] = directionalShadowMatrix[0] *
      (worldPosition + vec4(vWorldNormal * directionalLightShadows[0].shadowNormalBias, 0.0));
  #endif
  vSurfaceState = vec3(projected.crest / max(projected.slopeEnergy, 0.001),
    clamp(1.0 - horizontalDeterminant, 0.0, 1.0), aGridSpacing);
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeFilteredWaterVertexShader = /* glsl */ `
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <shadowmap_pars_vertex>
void main() {
  vec4 worldPosition = modelMatrix * vec4(position, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * vec3(0.0, 1.0, 0.0));
  vSurfaceState = vec3(0.0, 0.0, aGridSpacing);
  vSurfaceGradient = vec4(worldPosition.xz, 0.0, 0.0);
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    vDirectionalShadowCoord[0] = directionalShadowMatrix[0] *
      (worldPosition + vec4(vWorldNormal * directionalLightShadows[0].shadowNormalBias, 0.0));
  #endif
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeWaterFragmentShader = /* glsl */ `
uniform float uWindSpeed;
uniform vec4 uWindVelocity;
uniform float uDetailScale;
uniform float uVisualWaveHeight;
uniform float uTimeSeconds;
#if LAKE_WAVE_COUNT > 0
uniform vec4 uWaveKAmplitude[LAKE_WAVE_COUNT];
uniform vec4 uWaveOmegaPhase[LAKE_WAVE_COUNT];
#endif
uniform vec3 uWaterDark;
uniform vec3 uWaterMid;
uniform vec3 uWaterLight;
uniform vec3 uHazeColor;
uniform vec4 uHazeRange;
uniform vec4 uSunDirection;
uniform sampler2D uReflectionTexture;
uniform float uReflectionEnabled;
uniform mat4 uReflectionMatrix;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform sampler2D uSkyTexture;
uniform vec4 uDetailExtents;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <common>
#include <shadowmap_pars_fragment>
${LAKE_WAVE_PACKET_GLSL}

vec3 lakeSkyRadiance(vec3 ray) {
  vec3 skyRay = vec3(ray.x, max(ray.y, 0.0), ray.z);
  skyRay = dot(skyRay, skyRay) < 1e-8 ? vec3(1.0, 0.0, 0.0) : normalize(skyRay);
  vec2 skyUv = vec2(atan(skyRay.z, skyRay.x) / 6.2831853 + 0.5,
    asin(skyRay.y) / 3.1415927 + 0.5);
  vec2 gradientX = dFdx(skyUv);
  vec2 gradientY = dFdy(skyUv);
  gradientX.x -= round(gradientX.x);
  gradientY.x -= round(gradientY.x);
  return textureGrad(uSkyTexture, skyUv, gradientX, gradientY).rgb;
}

float lakeNoise(vec2 point) {
  vec2 cell = floor(point);
  vec2 blend = fract(point);
  blend = blend * blend * (3.0 - 2.0 * blend);
  float a = fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
  float b = fract(sin(dot(cell + vec2(1.0, 0.0), vec2(127.1, 311.7))) * 43758.5453);
  float c = fract(sin(dot(cell + vec2(0.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  float d = fract(sin(dot(cell + vec2(1.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  return mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
}

vec2 lakeWaveSlope(vec2 worldXZ) {
  // Add only wave detail that the vertex grid cannot resolve. Derivative based
  // filtering fades subpixel wavelengths smoothly as they recede from the eye.
  float waveSlopeX = 0.0;
  float waveSlopeZ = 0.0;
  #if LAKE_WAVE_COUNT > 0
  #pragma unroll_loop_start
  for (int i = 0; i < LAKE_WAVE_LOOP_COUNT; i++) {
    {
      vec4 ka = uWaveKAmplitude[i];
      vec4 op = uWaveOmegaPhase[i];
      float k = ka.z;
      vec2 direction = ka.xy;
      mat3 modulation = lakeWaveModulation(worldXZ, direction, k, op.y);
      vec3 warp = modulation[1];
      float phase = k * dot(direction, worldXZ) + warp.x - op.x * uTimeSeconds + op.y;
      float geometricVisibility = 1.0 - smoothstep(1.3, 2.5, k * vSurfaceState.z);
      float pixelVisibility = 1.0 - smoothstep(2.0, 5.0, fwidth(phase));
      float normalWeight = (1.0 - geometricVisibility) * pixelVisibility * 0.65;
      if (normalWeight != 0.0) {
        vec2 phaseGradient = k * direction + warp.yz;
        float crestBasis = 0.5 + 0.5 * sin(phase);
        vec3 packet = modulation[0];
        vec2 gradient = ka.w * (packet.x * 3.0 * crestBasis * crestBasis * cos(phase) * phaseGradient
          + (2.0 * crestBasis * crestBasis * crestBasis - 0.625) * packet.yz) * normalWeight;
        waveSlopeX += gradient.x;
        waveSlopeZ += gradient.y;
      }
    }
  }
  #pragma unroll_loop_end
  #endif
  return vec2(waveSlopeX, waveSlopeZ);
}

uniform vec2 uDetailFilterMinification;

float lakeDetailSpread(sampler2D image, vec2 gradientX, vec2 gradientY) {
  vec2 dimensions = vec2(textureSize(image, 0));
  vec2 texelX = gradientX * dimensions;
  vec2 texelY = gradientY * dimensions;
  float denominator = sqrt(dot(texelX, texelX) + dot(texelY, texelY));
  float minorBound = 0.0;
  if (denominator > 0.0) {
    minorBound = abs(texelX.x * texelY.y - texelX.y * texelY.x) / denominator;
  }
  return 1.0 - smoothstep(uDetailFilterMinification.x, uDetailFilterMinification.y, minorBound);
}

vec4 lakeDetailSample(sampler2D image, vec2 uv, vec2 gradientX, vec2 gradientY,
    vec4 offsets, bool centered, int corner) {
  vec2 point = uv;
  if (!centered) {
    point = uv - offsets.xy - offsets.zw;
    if (corner == 1) point = uv + offsets.xy - offsets.zw;
    if (corner == 2) point = uv - offsets.xy + offsets.zw;
    if (corner == 3) point = uv + offsets.xy + offsets.zw;
  }
  return textureGrad(image, point, gradientX, gradientY);
}

vec3 lakeMicroDetail(vec2 worldXZ, vec2 footprintX, vec2 footprintY) {
  const float fineAmplitude = 0.4;
  const float rippleAmplitude = 0.1;
  vec2 drift = uWindVelocity.xy * uTimeSeconds;
  vec2 nearUv = (worldXZ - drift * 0.42) / uDetailExtents.x;
  vec2 nearX = footprintX / uDetailExtents.x;
  vec2 nearY = footprintY / uDetailExtents.x;
  // These compact gravity-wave bands use phase-speed ratios proportional to
  // sqrt(wavelength): far/near/fine/ripple = 0.89/0.42/0.26/0.18.
  vec2 farUv = (worldXZ - drift * 0.89) / uDetailExtents.y;
  vec2 farX = footprintX / uDetailExtents.y;
  vec2 farY = footprintY / uDetailExtents.y;
  // Each rotated octave advects along its rotated propagation direction.
  // Reusing the unrotated drift makes oblique crests slide sideways.
  vec2 fineDrift = vec2(
    0.6 * drift.x + 0.8 * drift.y,
    -0.8 * drift.x + 0.6 * drift.y
  ) * 0.26;
  vec2 finePoint = (worldXZ - fineDrift) * 2.6;
  vec2 fineRotated = vec2(
    0.6 * finePoint.x - 0.8 * finePoint.y,
    0.8 * finePoint.x + 0.6 * finePoint.y
  );
  mat2 fineRotation = mat2(vec2(0.6, 0.8), vec2(-0.8, 0.6));
  vec2 fineUv = fineRotated / uDetailExtents.x + vec2(0.217, 0.631);
  vec2 fineX = fineRotation * footprintX * (2.6 / uDetailExtents.x);
  vec2 fineY = fineRotation * footprintY * (2.6 / uDetailExtents.x);
  vec2 rippleDrift = vec2(
    0.8 * drift.x + 0.6 * drift.y,
    -0.6 * drift.x + 0.8 * drift.y
  ) * 0.18;
  vec2 ripplePoint = (worldXZ - rippleDrift) * 5.2;
  vec2 rippleRotated = vec2(
    0.8 * ripplePoint.x - 0.6 * ripplePoint.y,
    0.6 * ripplePoint.x + 0.8 * ripplePoint.y
  );
  mat2 rippleRotation = mat2(vec2(0.8, 0.6), vec2(-0.6, 0.8));
  vec2 rippleUv = rippleRotated / uDetailExtents.x + vec2(0.683, 0.173);
  vec2 rippleX = rippleRotation * footprintX * (5.2 / uDetailExtents.x);
  vec2 rippleY = rippleRotation * footprintY * (5.2 / uDetailExtents.x);
  float nearSpread = lakeDetailSpread(uDetailNear, nearX, nearY);
  float farSpread = lakeDetailSpread(uDetailFar, farX, farY);
  float fineSpread = lakeDetailSpread(uDetailNear, fineX, fineY);
  float rippleSpread = lakeDetailSpread(uDetailNear, rippleX, rippleY);
  vec4 nearOffsets = vec4(nearX, nearY) * (0.25 * nearSpread);
  vec4 farOffsets = vec4(farX, farY) * (0.25 * farSpread);
  vec4 fineOffsets = vec4(fineX, fineY) * (0.25 * fineSpread);
  vec4 rippleOffsets = vec4(rippleX, rippleY) * (0.25 * rippleSpread);
  vec4 nearSample = vec4(0.0);
  vec4 farSample = vec4(0.0);
  vec4 fineSample = vec4(0.0);
  vec4 rippleSample = vec4(0.0);
  vec3 filtered = vec3(0.0);
  for (int corner = 0; corner < 4; corner++) {
    if (corner == 0 || nearSpread != 0.0) nearSample = lakeDetailSample(uDetailNear, nearUv, nearX, nearY, nearOffsets, nearSpread == 0.0, corner);
    if (corner == 0 || farSpread != 0.0) farSample = lakeDetailSample(uDetailFar, farUv, farX, farY, farOffsets, farSpread == 0.0, corner);
    if (corner == 0 || fineSpread != 0.0) fineSample = lakeDetailSample(uDetailNear, fineUv, fineX, fineY, fineOffsets, fineSpread == 0.0, corner);
    if (corner == 0 || rippleSpread != 0.0) rippleSample = lakeDetailSample(uDetailNear, rippleUv, rippleX, rippleY, rippleOffsets, rippleSpread == 0.0, corner);
    vec2 nearSlope = (nearSample.rg * 255.0 - 128.0) / 127.0;
    vec2 farSlope = (farSample.rg * 255.0 - 128.0) / 127.0;
    vec2 fineRawSlope = (fineSample.rg * 255.0 - 128.0) / 127.0;
    vec2 rippleRawSlope = (rippleSample.rg * 255.0 - 128.0) / 127.0;
    vec2 fineSlope = vec2(
      0.6 * fineRawSlope.x + 0.8 * fineRawSlope.y,
      -0.8 * fineRawSlope.x + 0.6 * fineRawSlope.y
    ) * 2.6 * fineAmplitude;
    vec2 rippleSlope = vec2(
      0.8 * rippleRawSlope.x + 0.6 * rippleRawSlope.y,
      -0.6 * rippleRawSlope.x + 0.8 * rippleRawSlope.y
    ) * 5.2 * rippleAmplitude;
    vec2 microSlope = nearSlope * 0.6 + farSlope * 0.55 + fineSlope + rippleSlope;
    // Blue stores slope squared. Mipmaps preserve the variance of unresolved
    // wavelets even after their mean slope approaches zero.
    float nearVariance = max(nearSample.b / 2.5 - dot(nearSlope, nearSlope), 0.0);
    float farVariance = max(farSample.b / 2.5 - dot(farSlope, farSlope), 0.0);
    float fineVariance = max(fineSample.b / 2.5 - dot(fineRawSlope, fineRawSlope), 0.0);
    float rippleVariance = max(rippleSample.b / 2.5 - dot(rippleRawSlope, rippleRawSlope), 0.0);
    float slopeMoment = dot(microSlope, microSlope)
      + 0.6 * 0.6 * nearVariance + 0.55 * 0.55 * farVariance
      + 2.6 * 2.6 * fineAmplitude * fineAmplitude * fineVariance
      + 5.2 * 5.2 * rippleAmplitude * rippleAmplitude * rippleVariance;
    filtered += vec3(microSlope, slopeMoment) * 0.25;
  }
  float strength = smoothstep(0.05, 0.6, uWindSpeed);
  float scaledStrength = strength * uDetailScale;
  return vec3(filtered.xy * scaledStrength, filtered.z * scaledStrength * scaledStrength);
}

vec3 sampleLakeColor(vec2 worldXZ, vec2 waveSlope, vec3 microDetail, float sunVisibility) {
  vec3 geometricNormal = normalize(vWorldNormal) * (gl_FrontFacing ? 1.0 : -1.0);
  float patches = lakeNoise(worldXZ * 0.17 + vec2(2.3, -7.1));
  vec2 microSlope = microDetail.xy;
  float unresolvedVariance = max(microDetail.z - dot(microSlope, microSlope), 0.0);
  // Subtract the exact vertex slope already represented by the geometric
  // normal; vertex LOD0 and filtered fragment samples need not be identical.
  vec2 geometricSlope = -geometricNormal.xz / max(geometricNormal.y, 0.01);
  vec2 totalSlope = geometricSlope + waveSlope + microSlope - vSurfaceGradient.zw;
  vec3 normal = normalize(vec3(-totalSlope.x, 1.0, -totalSlope.y));
  vec3 viewDirection = normalize(cameraPosition - vec3(worldXZ.x, vWorldPosition.y, worldXZ.y));
  float ndv = max(dot(geometricNormal, viewDirection), 0.0);
  float geometricFresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
  float facetNdotV = dot(normal, viewDirection);
  float facetFresnel = 0.02 + 0.98 * pow(1.0 - max(facetNdotV, 0.0), 5.0);
  float rayVisibility = smoothstep(0.0, 0.18, facetNdotV);
  float fresnelVisibility = rayVisibility * smoothstep(0.03, 0.22, ndv);
  vec3 reflectedDirection = normalize(mix(reflect(-viewDirection, geometricNormal),
    reflect(-viewDirection, normal), rayVisibility));
  vec3 reflection = lakeSkyRadiance(reflectedDirection);
  float fresnel = mix(geometricFresnel, facetFresnel, 0.4 * fresnelVisibility);
  float sunAlignment = max(dot(reflectedDirection, normalize(uSunDirection.xyz)), 0.0);
  float roughness = clamp(0.23 + uWindSpeed * 0.018 + sqrt(unresolvedVariance) * 0.6, 0.23, 0.62);
  float glitterLobe = pow(sunAlignment, mix(220.0, 45.0, roughness));
  float glitterNoise = 0.35 + 0.65 * clamp(sqrt(microDetail.z) * 3.0, 0.0, 1.0);
  float glitter = glitterLobe * glitterNoise * smoothstep(0.35, 2.0, uWindSpeed) * sunVisibility * uSunDirection.w;
  float diffuse = 0.58 + 0.42 * sunVisibility * max(dot(normal, normalize(uSunDirection.xyz)), 0.0) * uSunDirection.w;
  float sharpCrest = smoothstep(0.42, 0.76, vSurfaceState.y);
  float crestLight = smoothstep(0.05, 1.05, vSurfaceState.x) * sharpCrest * mix(0.35, 1.0, patches);
  vec3 base = mix(uWaterDark, uWaterMid, diffuse * 0.72);
  base = mix(base, uWaterLight, crestLight * 0.16 + patches * 0.07);
  base *= mix(0.82, 1.0, sunVisibility);
  // Existing wave groups may remain steep after the local wind weakens.
  float whitecap = smoothstep(0.68, 0.84, vSurfaceState.y) * smoothstep(0.7, 0.98, vSurfaceState.x) * 0.12;
  // Dark water absorption must not also darken the reflected sky.
  base *= vec3(0.29, 0.34, 0.37);
  // Art-directed reflection strength for the dark reference water. This is
  // not a rough-surface BRDF and leaves a visible sky/water boundary.
  vec3 color = mix(base, reflection, fresnel * 0.45);
  if (uReflectionEnabled > 0.5) {
    vec4 reflectionCoord = uReflectionMatrix * vec4(vWorldPosition, 1.0);
    if (reflectionCoord.w > 0.0) {
      vec2 reflectedUv = reflectionCoord.xy / reflectionCoord.w * 0.5 + 0.5;
    // Small surface-slope offsets break up the image along moving wave facets.
      reflectedUv += totalSlope * 0.006;
      if (all(greaterThanEqual(reflectedUv, vec2(0.0))) &&
          all(lessThanEqual(reflectedUv, vec2(1.0)))) {
        vec4 aircraftReflection = textureLod(uReflectionTexture, reflectedUv, 0.0);
        color = mix(color, aircraftReflection.rgb,
          aircraftReflection.a * clamp(0.16 + fresnel * 0.7, 0.0, 0.72));
      }
    }
  }
  float depthTint = clamp(uVisualWaveHeight / 0.35, 0.0, 1.0);
  color = mix(color, color * vec3(0.92, 0.97, 1.04), depthTint * 0.12);
  color += vec3(1.0, 0.78, 0.52) * glitter * 0.15;
  color = mix(color, vec3(0.38, 0.43, 0.43), whitecap * 0.28);
  // Use the scene's same fog curve and sky-colored airlight so distant water
  // fades into the far shore without a separate dark horizon band.
  float distanceToEye = length(cameraPosition - vWorldPosition);
  float atmosphericFade = smoothstep(uHazeRange.x, uHazeRange.y, distanceToEye);
  color = mix(color, uHazeColor, atmosphericFade);
  return color;
}

void main() {
  vec2 footprintX = dFdx(vSurfaceGradient.xy);
  vec2 footprintY = dFdy(vSurfaceGradient.xy);
  vec2 center = vSurfaceGradient.xy;
  vec2 waveSlope = lakeWaveSlope(center);
  vec3 microDetail = lakeMicroDetail(center, footprintX, footprintY);
  float sunVisibility = 1.0;
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    sunVisibility = getShadow(directionalShadowMap[0], directionalLightShadows[0].shadowMapSize,
      directionalLightShadows[0].shadowIntensity, directionalLightShadows[0].shadowBias,
      directionalLightShadows[0].shadowRadius, vDirectionalShadowCoord[0]);
  #endif
  vec3 color = sampleLakeColor(vWorldPosition.xz, waveSlope, microDetail, sunVisibility);
  gl_FragColor = vec4(color, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export function lakeWaterShaders(waveCount: number) {
  const count = String(waveCount);
  return {
    vertexShader: lakeWaterVertexShader.replaceAll("LAKE_WAVE_LOOP_COUNT", count),
    filteredVertexShader: lakeFilteredWaterVertexShader,
    fragmentShader: lakeWaterFragmentShader.replaceAll("LAKE_WAVE_LOOP_COUNT", count)
  };
}

