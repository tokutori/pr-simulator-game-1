import {
  BufferGeometry,
  CylinderGeometry,
  DataTexture,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  LinearFilter,
  LinearMipmapLinearFilter,
  Mesh,
  MeshLambertMaterial,
  RGBAFormat,
  SRGBColorSpace,
  UnsignedByteType,
  Vector3
} from "three";

// tokutori_2026 three-view reference: measured tip-to-tip span is 20,863 mm.
const HALF_SPAN_METERS = 20.863 / 2;
// Chords, tail span, and longitudinal stations are scaled estimates from the
// supplied orthographic images; only the full wing span has a stated dimension.
const WING_ROOT_CHORD_METERS = 1.10;
const WING_TIP_CHORD_METERS = 0.40;
const WING_ROOT_LEADING_Z = -0.42;
const WING_LEADING_SWEEP_METERS = (WING_ROOT_CHORD_METERS - WING_TIP_CHORD_METERS) / 2;
const WING_ROOT_SPAR_Z = WING_ROOT_LEADING_Z + 0.25 * WING_ROOT_CHORD_METERS;
const TAIL_HALF_SPAN_METERS = 1.58;
const TAIL_ROOT_CHORD_METERS = 0.55;
const TAIL_TIP_CHORD_METERS = 0.48;
const TAIL_LEADING_SWEEP_METERS = 0.03;
const TAIL_Z_METERS = 4.25;
const ELEVATOR_HINGE_FRACTION = 0.7;
const ELEVATOR_HINGE_Z = TAIL_ROOT_CHORD_METERS * ELEVATOR_HINGE_FRACTION;
const REFERENCE_AIRSPEED_METERS_PER_SECOND = 9.5;
const YOUNGS_MODULUS_PASCALS = 45e9;
const SPAR_SECOND_MOMENT_METERS_FOURTH = 4e-6;
const VISUAL_SUPPORTED_MASS_KILOGRAMS = 110;
const GRAVITY_METERS_PER_SECOND_SQUARED = 9.81;

interface AirfoilHalf {
  readonly mesh: Mesh<BufferGeometry, MeshLambertMaterial>;
  readonly baseY: Float32Array;
  readonly span: Float32Array;
}

interface AirfoilShape {
  readonly side: -1 | 1;
  readonly halfSpan: number;
  readonly rootChord: number;
  readonly tipChord: number;
  readonly leadingZ: number;
  readonly sweep: number;
  readonly baseY: number;
  readonly dihedral: number;
  readonly camber: number;
  readonly camberPosition: number;
  readonly thickness: number;
  readonly chordStart: number;
  readonly chordEnd: number;
  readonly flex: boolean;
}

function mainWingChord(span: number): number {
  return WING_ROOT_CHORD_METERS +
    (WING_TIP_CHORD_METERS - WING_ROOT_CHORD_METERS) * span / HALF_SPAN_METERS;
}

function mainWingLeadingZ(span: number): number {
  return WING_ROOT_LEADING_Z + WING_LEADING_SWEEP_METERS * span / HALF_SPAN_METERS;
}

function tailChord(span: number): number {
  return TAIL_ROOT_CHORD_METERS +
    (TAIL_TIP_CHORD_METERS - TAIL_ROOT_CHORD_METERS) * span / TAIL_HALF_SPAN_METERS;
}

function indigoWingTexture(): DataTexture {
  const width = 1024;
  const height = 256;
  const pixels = new Uint8Array(width * height * 4);
  const smooth = (from: number, to: number, value: number): number => {
    const t = Math.max(0, Math.min(1, (value - from) / (to - from)));
    return t * t * (3 - 2 * t);
  };
  for (let y = 0; y < height; y++) {
    const chord = (y + 0.5) / height;
    for (let x = 0; x < width; x++) {
      const span = (x + 0.5) / width;
      const grain = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
      const weave = (grain - Math.floor(grain) - 0.5) * 11;
      const dye = 5 * Math.sin(span * 83 + chord * 17) + 4 * Math.sin(span * 163 - chord * 31);
      let white = 0;
      for (let ring = 0; ring < 8; ring++) {
        const center = (ring + 0.54) / 8;
        const along = (span - center) / 0.038;
        const across = (chord - 0.27) / 0.155;
        const angle = Math.atan2(across, along);
        const radius = Math.hypot(along, across)
          + 0.065 * Math.sin(angle * 7 + ring * 1.3)
          + 0.04 * Math.sin(angle * 13 - ring * 0.7);
        const ringBand = smooth(0.73, 0.81, radius) * (1 - smooth(1.07, 1.16, radius));
        white = Math.max(white, ringBand);
      }
      const index = (y * width + x) * 4;
      pixels[index] = Math.round((42 + dye + weave) * (1 - white) + (218 + weave * 0.4) * white);
      pixels[index + 1] = Math.round((63 + dye + weave) * (1 - white) + (224 + weave * 0.4) * white);
      pixels[index + 2] = Math.round((86 + dye + weave) * (1 - white) + (218 + weave * 0.4) * white);
      pixels[index + 3] = 255;
    }
  }
  const texture = new DataTexture(pixels, width, height, RGBAFormat, UnsignedByteType);
  texture.colorSpace = SRGBColorSpace;
  texture.magFilter = LinearFilter;
  texture.minFilter = LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.needsUpdate = true;
  return texture;
}

export interface BirdmanAirframe {
  readonly root: Group;
  setVisualState(airspeedMetersPerSecond: number | null, elevatorRadians: number, rudderRadians: number): void;
  dispose(): void;
}

/** Visual cantilever approximation; these assumptions do not enter flight physics. */
export function wingDeflectionMeters(spanMeters: number, airspeedMetersPerSecond: number): number {
  const span = Math.max(0, Math.min(HALF_SPAN_METERS, spanMeters));
  const speed = Number.isFinite(airspeedMetersPerSecond)
    ? Math.max(0, airspeedMetersPerSecond) : REFERENCE_AIRSPEED_METERS_PER_SECOND;
  // Speed is a bounded visual load proxy because the render pose has no spanwise
  // lift distribution. One reference g supports the chosen pilot and airframe mass.
  const loadFactor = Math.max(0.3, Math.min(1.6,
    (speed / REFERENCE_AIRSPEED_METERS_PER_SECOND) ** 2));
  const loadPerHalfSpan = VISUAL_SUPPORTED_MASS_KILOGRAMS * GRAVITY_METERS_PER_SECOND_SQUARED
    * loadFactor / (2 * HALF_SPAN_METERS);
  const length = HALF_SPAN_METERS;
  return loadPerHalfSpan * span * span *
    (6 * length * length - 4 * length * span + span * span) /
    (24 * YOUNGS_MODULUS_PASCALS * SPAR_SECOND_MOMENT_METERS_FOURTH);
}

export function wingDihedralMeters(spanMeters: number): number {
  const span = Math.max(0, Math.min(HALF_SPAN_METERS, spanMeters));
  const firstBreak = 0.6 * HALF_SPAN_METERS;
  const secondBreak = 0.8 * HALF_SPAN_METERS;
  return 0.008 * Math.min(span, firstBreak) +
    0.045 * Math.max(0, Math.min(span, secondBreak) - firstBreak) +
    0.12 * Math.max(0, span - secondBreak);
}

function sectionProfile(chordFraction: number, camber: number, camberPosition: number, thickness: number):
  Readonly<{ upperX: number; upperY: number; lowerX: number; lowerY: number }> {
  const x = chordFraction;
  const p = camberPosition;
  const mean = x < p ? camber * (2 * p * x - x * x) / (p * p)
    : camber * ((1 - 2 * p) + 2 * p * x - x * x) / ((1 - p) * (1 - p));
  const slope = x < p ? 2 * camber * (p - x) / (p * p)
    : 2 * camber * (p - x) / ((1 - p) * (1 - p));
  const halfThickness = 5 * thickness *
    (0.2969 * Math.sqrt(x) - 0.1260 * x - 0.3516 * x * x +
      0.2843 * x ** 3 - 0.1036 * x ** 4);
  const angle = Math.atan(slope);
  return {
    upperX: x - halfThickness * Math.sin(angle),
    upperY: mean + halfThickness * Math.cos(angle),
    lowerX: x + halfThickness * Math.sin(angle),
    lowerY: mean - halfThickness * Math.cos(angle)
  };
}

function airfoilHalf(shape: AirfoilShape, material: MeshLambertMaterial): AirfoilHalf {
  // Thirty sections place vertices exactly at the 60% and 80% dihedral breaks.
  const spanSteps = shape.flex ? 30 : 10;
  const chordSteps = 16;
  const rowSize = chordSteps + 1;
  const surfaceSize = (spanSteps + 1) * rowSize;
  const vertices: number[] = [];
  const uvs: number[] = [];
  const baseY: number[] = [];
  const span: number[] = [];
  const indices: number[] = [];
  for (const upper of [true, false]) {
    for (let section = 0; section <= spanSteps; section++) {
      const fraction = section / spanSteps;
      const distance = shape.halfSpan * fraction;
      const chord = shape.rootChord + (shape.tipChord - shape.rootChord) * fraction;
      const leadingZ = shape.leadingZ + shape.sweep * fraction;
      for (let station = 0; station <= chordSteps; station++) {
        const distribution = (1 - Math.cos(Math.PI * station / chordSteps)) * 0.5;
        const x = shape.chordStart + (shape.chordEnd - shape.chordStart) * distribution;
        const profile = sectionProfile(x, shape.camber, shape.camberPosition, shape.thickness);
        const y = shape.baseY + (shape.flex ? wingDihedralMeters(distance) : shape.dihedral * distance) +
          (upper ? profile.upperY : profile.lowerY) * chord;
        const z = leadingZ + (upper ? profile.upperX : profile.lowerX) * chord;
        vertices.push(shape.side * distance, y, z);
        uvs.push(fraction, x);
        baseY.push(y);
        span.push(shape.flex ? distance : 0);
      }
    }
  }
  for (let surface = 0; surface < 2; surface++) {
    const upper = surface === 0;
    const base = surface * surfaceSize;
    for (let section = 0; section < spanSteps; section++) {
      for (let station = 0; station < chordSteps; station++) {
        const a = base + section * rowSize + station;
        const b = a + rowSize;
        const c = a + 1;
        const d = b + 1;
        if ((shape.side === 1) === upper) indices.push(a, c, b, c, d, b);
        else indices.push(a, b, c, c, b, d);
      }
    }
  }
  // Close the exposed root, tip, and hinge edges of a cropped airfoil.
  const lowerOffset = surfaceSize;
  for (let section = 0; section < spanSteps; section++) {
    for (const station of [0, chordSteps]) {
      const a = section * rowSize + station;
      const b = a + rowSize;
      indices.push(a, b, lowerOffset + a, b, lowerOffset + b, lowerOffset + a);
    }
  }
  for (const section of [0, spanSteps]) {
    for (let station = 0; station < chordSteps; station++) {
      const a = section * rowSize + station;
      const b = a + 1;
      indices.push(a, lowerOffset + a, b, b, lowerOffset + a, lowerOffset + b);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(vertices, 3));
  geometry.setAttribute("uv", new Float32BufferAttribute(uvs, 2));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  const mesh = new Mesh(geometry, material);
  mesh.frustumCulled = false;
  return { mesh, baseY: Float32Array.from(baseY), span: Float32Array.from(span) };
}

function finGeometry(outline: readonly (readonly [number, number])[], halfThickness: number): BufferGeometry {
  const points: number[] = [];
  for (const x of [-halfThickness, halfThickness]) {
    for (const [y, z] of outline) points.push(x, y, z);
  }
  const indices: number[] = [];
  const n = outline.length;
  for (let index = 1; index < n - 1; index++) {
    indices.push(0, index + 1, index, n, n + index, n + index + 1);
  }
  for (let index = 0; index < n; index++) {
    const next = (index + 1) % n;
    indices.push(index, next, n + index, next, n + next, n + index);
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(points, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

interface CanopyStation {
  readonly z: number;
  readonly centerY: number;
  readonly halfWidth: number;
  readonly halfHeight: number;
}

function canopyPoint(station: CanopyStation, angle: number): Vector3 {
  const rounded = (value: number): number => Math.sign(value) * Math.abs(value) ** 0.8;
  return new Vector3(
    station.halfWidth * rounded(Math.cos(angle)),
    station.centerY + station.halfHeight * rounded(Math.sin(angle)),
    station.z
  );
}

function canopyGeometry(stations: readonly CanopyStation[]): BufferGeometry {
  const circumferenceSteps = 24;
  const rowSize = circumferenceSteps + 1;
  const vertices: number[] = [];
  const indices: number[] = [];
  for (const station of stations) {
    for (let step = 0; step <= circumferenceSteps; step++) {
      vertices.push(...canopyPoint(station, step * Math.PI * 2 / circumferenceSteps).toArray());
    }
  }
  for (let section = 0; section < stations.length - 1; section++) {
    for (let step = 0; step < circumferenceSteps; step++) {
      const a = section * rowSize + step;
      const b = a + rowSize;
      indices.push(a, a + 1, b, a + 1, b + 1, b);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(vertices, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

function pointRod(rod: Mesh, from: Vector3, to: Vector3): void {
  const vector = to.clone().sub(from);
  rod.position.copy(from).add(to).multiplyScalar(0.5);
  rod.scale.y = vector.length();
  rod.quaternion.setFromUnitVectors(new Vector3(0, 1, 0), vector.normalize());
}

function addRod(parent: Group, from: Vector3, to: Vector3, radius: number, material: MeshLambertMaterial): Mesh {
  const rod = new Mesh(new CylinderGeometry(radius, radius, 1, 8), material);
  pointRod(rod, from, to);
  parent.add(rod);
  return rod;
}

function wingRibPoint(side: -1 | 1, span: number, chordFraction: number, speed: number): Vector3 {
  const chord = mainWingChord(span);
  const profile = sectionProfile(chordFraction, 0.04, 0.4, 0.12);
  const meanY = (profile.upperY + profile.lowerY) * 0.5;
  return new Vector3(
    side * span,
    -0.73 + wingDihedralMeters(span) + meanY * chord + wingDeflectionMeters(span, speed),
    mainWingLeadingZ(span) + chordFraction * chord
  );
}

function wingSparPoint(side: -1 | 1, span: number, speed: number): Vector3 {
  const chord = mainWingChord(span);
  return new Vector3(
    side * span,
    -0.73 + wingDihedralMeters(span) + wingDeflectionMeters(span, speed),
    mainWingLeadingZ(span) + 0.25 * chord
  );
}

function foamRibGeometry(span: number): BufferGeometry {
  const chord = mainWingChord(span);
  const leadingZ = mainWingLeadingZ(span);
  const baseY = -0.73 + wingDihedralMeters(span);
  const stations = [0.58, 0.80, 0.995];
  const outline: [number, number][] = [];
  for (const chordFraction of stations) {
    const profile = sectionProfile(chordFraction, 0.04, 0.4, 0.12);
    outline.push([baseY + profile.upperY * chord, leadingZ + profile.upperX * chord]);
  }
  for (const chordFraction of [...stations].reverse()) {
    const profile = sectionProfile(chordFraction, 0.04, 0.4, 0.12);
    outline.push([baseY + profile.lowerY * chord, leadingZ + profile.lowerX * chord]);
  }
  return finGeometry(outline, 0.013);
}

function tailRibPoint(side: -1 | 1, span: number, chordFraction: number): Vector3 {
  const fraction = span / TAIL_HALF_SPAN_METERS;
  const chord = tailChord(span);
  return new Vector3(
    side * span,
    0.02 * span,
    -ELEVATOR_HINGE_Z + TAIL_LEADING_SWEEP_METERS * fraction + chordFraction * chord
  );
}

function tailFoamRibGeometry(span: number): BufferGeometry {
  const fraction = span / TAIL_HALF_SPAN_METERS;
  const chord = tailChord(span);
  const leadingZ = -ELEVATOR_HINGE_Z + TAIL_LEADING_SWEEP_METERS * fraction;
  const baseY = 0.02 * span;
  const stations = [0.70, 0.85, 0.995];
  const outline: [number, number][] = [];
  for (const chordFraction of stations) {
    const profile = sectionProfile(chordFraction, 0, 0.4, 0.09);
    outline.push([baseY + profile.upperY * chord, leadingZ + profile.upperX * chord]);
  }
  for (const chordFraction of [...stations].reverse()) {
    const profile = sectionProfile(chordFraction, 0, 0.4, 0.09);
    outline.push([baseY + profile.lowerY * chord, leadingZ + profile.lowerX * chord]);
  }
  return finGeometry(outline, 0.008);
}

export function createBirdmanAirframe(): BirdmanAirframe {
  const root = new Group();
  root.name = "birdman-airframe";
  const wingPattern = indigoWingTexture();
  const wingSkin = new MeshLambertMaterial({ color: 0xffffff, map: wingPattern, side: DoubleSide });
  const skin = new MeshLambertMaterial({ color: 0x304c6b, side: DoubleSide });
  const trailingFilm = new MeshLambertMaterial({
    color: 0xd5ddd9, side: DoubleSide, transparent: true, opacity: 0.22, depthWrite: false
  });
  const ribMaterial = new MeshLambertMaterial({ color: 0xcdb483 });
  const styrofoam = new MeshLambertMaterial({ color: 0x77b5d0, side: DoubleSide });
  const carbon = new MeshLambertMaterial({ color: 0x303b3a });
  const whiteFrame = new MeshLambertMaterial({ color: 0xe4e7e3 });
  const canopyFilm = new MeshLambertMaterial({
    color: 0xd6e3e5, side: DoubleSide, transparent: true, opacity: 0.23, depthWrite: false
  });
  const canopyRim = new MeshLambertMaterial({ color: 0xb8c5c7 });
  const fairingSkin = new MeshLambertMaterial({ color: 0xe1e3db, side: DoubleSide });
  const wings: AirfoilHalf[] = [];
  const wingSpar = new Group();
  wingSpar.name = "main-wing-spar";
  root.add(wingSpar);
  const sparSegments: { rod: Mesh; side: -1 | 1; from: number; to: number }[] = [];
  const ribs: { rod: Mesh; side: -1 | 1; span: number; from: number; to: number }[] = [];
  const foamRibs: { mesh: Mesh; span: number }[] = [];
  const stringers: { rod: Mesh; side: -1 | 1; from: number; to: number; chordFraction: number }[] = [];
  for (const side of [-1, 1] as const) {
    const wingShape = {
      side, halfSpan: HALF_SPAN_METERS, rootChord: WING_ROOT_CHORD_METERS, tipChord: WING_TIP_CHORD_METERS,
      leadingZ: WING_ROOT_LEADING_Z, sweep: WING_LEADING_SWEEP_METERS, baseY: -0.73, dihedral: 0,
      camber: 0.04, camberPosition: 0.4, thickness: 0.12,
      flex: true
    };
    const leadingWing = airfoilHalf({ ...wingShape, chordStart: 0, chordEnd: 0.58 }, wingSkin);
    leadingWing.mesh.name = side === -1 ? "left-wing" : "right-wing";
    root.add(leadingWing.mesh);
    const film = airfoilHalf({ ...wingShape, chordStart: 0.58, chordEnd: 1 }, trailingFilm);
    film.mesh.name = side === -1 ? "left-trailing-film" : "right-trailing-film";
    root.add(film.mesh);
    wings.push(leadingWing, film);
    for (let section = 0; section < 12; section++) {
      const from = HALF_SPAN_METERS * section / 12;
      const to = HALF_SPAN_METERS * (section + 1) / 12;
      const radius = 0.055 + (0.018 - 0.055) * (section + 0.5) / 12;
      const rod = addRod(wingSpar,
        wingSparPoint(side, from, REFERENCE_AIRSPEED_METERS_PER_SECOND),
        wingSparPoint(side, to, REFERENCE_AIRSPEED_METERS_PER_SECOND),
        radius, carbon);
      sparSegments.push({ rod, side, from, to });
    }
    for (let ribIndex = 1; ribIndex <= 22; ribIndex++) {
      const span = HALF_SPAN_METERS * ribIndex / 23;
      const foam = new Mesh(foamRibGeometry(span), styrofoam);
      foam.name = side === -1 ? "left-styrofoam-rib" : "right-styrofoam-rib";
      foam.position.set(side * span, wingDeflectionMeters(span, REFERENCE_AIRSPEED_METERS_PER_SECOND), 0);
      root.add(foam);
      foamRibs.push({ mesh: foam, span });
      for (const [from, to] of [[0.58, 0.79], [0.79, 1]] as const) {
        const rod = addRod(root,
          wingRibPoint(side, span, from, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          wingRibPoint(side, span, to, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          0.012, ribMaterial);
        rod.name = side === -1 ? "left-trailing-rib" : "right-trailing-rib";
        ribs.push({ rod, side, span, from, to });
      }
    }
    for (const chordFraction of [0.58, 0.995]) {
      for (let section = 0; section < 12; section++) {
        const from = HALF_SPAN_METERS * section / 12;
        const to = HALF_SPAN_METERS * (section + 1) / 12;
        const rod = addRod(root,
          wingRibPoint(side, from, chordFraction, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          wingRibPoint(side, to, chordFraction, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          0.006, ribMaterial);
        rod.name = "balsa-stringer";
        stringers.push({ rod, side, from, to, chordFraction });
      }
    }
  }
  const cockpitFrame = new Group();
  cockpitFrame.name = "open-cockpit-frame";
  root.add(cockpitFrame);
  for (const side of [-1, 1] as const) {
    const upperFront = new Vector3(side * 0.22, -1.05, -0.75);
    const upperRear = new Vector3(side * 0.22, -1.02, 0.52);
    const lowerFront = new Vector3(side * 0.18, -1.42, -0.65);
    const lowerRear = new Vector3(side * 0.18, -1.39, 0.35);
    addRod(cockpitFrame, new Vector3(0, -1.22, -1.25), upperFront, 0.012, whiteFrame);
    addRod(cockpitFrame, upperFront, upperRear, 0.012, whiteFrame);
    addRod(cockpitFrame, lowerFront, lowerRear, 0.012, whiteFrame);
    addRod(cockpitFrame, upperFront, lowerFront, 0.008, whiteFrame);
    addRod(cockpitFrame, upperRear, lowerRear, 0.008, whiteFrame);
    addRod(cockpitFrame, lowerFront, upperRear, 0.007, whiteFrame);
  }
  for (const z of [-0.65, 0.35]) {
    addRod(cockpitFrame, new Vector3(-0.18, -1.40, z),
      new Vector3(0.18, -1.40, z), 0.008, whiteFrame);
  }
  const mainBeam = addRod(root,
    new Vector3(0, -0.73, WING_ROOT_SPAR_Z), new Vector3(0, -0.57, 4.75), 0.075, carbon);
  mainBeam.name = "single-carbon-main-beam";
  const clearCanopyStations: readonly CanopyStation[] = [
    { z: -1.25, centerY: -1.04, halfWidth: 0.025, halfHeight: 0.045 },
    { z: -1.10, centerY: -1.03, halfWidth: 0.09, halfHeight: 0.13 },
    { z: -0.92, centerY: -1.02, halfWidth: 0.17, halfHeight: 0.22 },
    { z: -0.70, centerY: -1.01, halfWidth: 0.24, halfHeight: 0.31 },
    { z: -0.48, centerY: -1.00, halfWidth: 0.28, halfHeight: 0.35 },
    { z: -0.25, centerY: -0.99, halfWidth: 0.31, halfHeight: 0.37 }
  ];
  const canopy = new Mesh(canopyGeometry(clearCanopyStations), canopyFilm);
  canopy.name = "transparent-canopy";
  canopy.frustumCulled = false;
  root.add(canopy);
  const rearFairing = new Mesh(canopyGeometry([
    { z: -0.25, centerY: -0.99, halfWidth: 0.31, halfHeight: 0.37 },
    { z: 0.05, centerY: -0.96, halfWidth: 0.33, halfHeight: 0.40 },
    { z: 0.45, centerY: -0.93, halfWidth: 0.29, halfHeight: 0.36 },
    { z: 0.86, centerY: -0.87, halfWidth: 0.19, halfHeight: 0.23 },
    { z: 1.22, centerY: -0.82, halfWidth: 0.025, halfHeight: 0.04 }
  ]), fairingSkin);
  rearFairing.name = "rear-cockpit-fairing";
  root.add(rearFairing);
  const frontRim = clearCanopyStations[clearCanopyStations.length - 1];
  if (frontRim !== undefined) {
    for (let step = 0; step < 16; step++) {
      addRod(root,
        canopyPoint(frontRim, step * Math.PI * 2 / 16),
        canopyPoint(frontRim, (step + 1) * Math.PI * 2 / 16),
        0.009, canopyRim);
    }
  }

  const tail = new Group();
  tail.position.set(0, -0.52, TAIL_Z_METERS);
  root.add(tail);
  for (const side of [-1, 1] as const) {
    const basis = {
      side, halfSpan: TAIL_HALF_SPAN_METERS, rootChord: TAIL_ROOT_CHORD_METERS, tipChord: TAIL_TIP_CHORD_METERS,
      leadingZ: 0, sweep: TAIL_LEADING_SWEEP_METERS, baseY: 0, dihedral: 0.02,
      camber: 0, camberPosition: 0.4, thickness: 0.09, flex: false
    };
    const stabilizer = airfoilHalf({ ...basis, chordStart: 0, chordEnd: 0.7 }, skin);
    stabilizer.mesh.name = side === -1 ? "left-tail-leading" : "right-tail-leading";
    tail.add(stabilizer.mesh);
  }
  const elevator = new Group();
  elevator.name = "elevator";
  elevator.position.set(0, 0, ELEVATOR_HINGE_Z);
  tail.add(elevator);
  for (const side of [-1, 1] as const) {
    const half = airfoilHalf({
      side, halfSpan: TAIL_HALF_SPAN_METERS, rootChord: TAIL_ROOT_CHORD_METERS, tipChord: TAIL_TIP_CHORD_METERS,
      leadingZ: -ELEVATOR_HINGE_Z, sweep: TAIL_LEADING_SWEEP_METERS, baseY: 0, dihedral: 0.02,
      camber: 0, camberPosition: 0.4, thickness: 0.09,
      chordStart: ELEVATOR_HINGE_FRACTION, chordEnd: 1, flex: false
    }, trailingFilm);
    half.mesh.name = side === -1 ? "left-elevator-film" : "right-elevator-film";
    elevator.add(half.mesh);
    for (let ribIndex = 1; ribIndex <= 8; ribIndex++) {
      const span = TAIL_HALF_SPAN_METERS * ribIndex / 9;
      const foam = new Mesh(tailFoamRibGeometry(span), styrofoam);
      foam.name = "elevator-styrofoam-rib";
      foam.position.x = side * span;
      elevator.add(foam);
      const rod = addRod(elevator,
        tailRibPoint(side, span, ELEVATOR_HINGE_FRACTION),
        tailRibPoint(side, span, 0.995),
        0.008, ribMaterial);
      rod.name = "elevator-rib";
    }
    for (let section = 0; section < 4; section++) {
      const from = TAIL_HALF_SPAN_METERS * section / 4;
      const to = TAIL_HALF_SPAN_METERS * (section + 1) / 4;
      const rod = addRod(elevator,
        tailRibPoint(side, from, 0.995), tailRibPoint(side, to, 0.995),
        0.006, ribMaterial);
      rod.name = "elevator-balsa-stringer";
    }
  }
  const rudder = new Group();
  rudder.name = "rudder";
  tail.add(rudder);
  const finFront = new Mesh(finGeometry([
    [0, 0.02], [1.33, 0.25], [1.52, 0.36], [0, 0.39]
  ], 0.045), skin);
  finFront.name = "rudder-front";
  rudder.add(finFront);
  const finFilm = new Mesh(finGeometry([
    [0, 0.39], [1.52, 0.36], [1.52, 0.53], [0, 0.62]
  ], 0.036), trailingFilm);
  finFilm.name = "rudder-trailing-film";
  rudder.add(finFilm);
  for (let ribIndex = 1; ribIndex <= 8; ribIndex++) {
    const y = 1.52 * ribIndex / 9;
    const fraction = y / 1.52;
    const rod = addRod(rudder,
      new Vector3(0, y, 0.39 - 0.03 * fraction),
      new Vector3(0, y, 0.62 - 0.09 * fraction),
      0.009, ribMaterial);
    rod.name = "rudder-rib";
  }
  for (let bay = 0; bay < 8; bay++) {
    const lowY = bay * 1.52 / 8;
    const highY = (bay + 1) * 1.52 / 8;
    const frontZ = (y: number): number => 0.39 - 0.03 * y / 1.52;
    const backZ = (y: number): number => 0.62 - 0.09 * y / 1.52;
    const foam = new Mesh(finGeometry([
      [lowY, frontZ(lowY)], [highY, frontZ(highY)], [lowY, backZ(lowY)]
    ], 0.012), styrofoam);
    foam.name = "rudder-styrofoam-panel";
    rudder.add(foam);
  }
  const rudderStringer = addRod(rudder,
    new Vector3(0, 0, 0.62), new Vector3(0, 1.52, 0.53),
    0.006, ribMaterial);
  rudderStringer.name = "rudder-balsa-stringer";

  let currentSpeed = Number.NaN;
  const setVisualState = (airspeedMetersPerSecond: number | null, elevatorRadians: number, rudderRadians: number): void => {
    const speed = airspeedMetersPerSecond ?? REFERENCE_AIRSPEED_METERS_PER_SECOND;
    if (!Number.isFinite(currentSpeed) || Math.abs(speed - currentSpeed) > 0.03) {
      currentSpeed = speed;
      for (const wing of wings) {
        const position = wing.mesh.geometry.getAttribute("position");
        for (let vertex = 0; vertex < wing.baseY.length; vertex++) {
          position.setY(vertex, (wing.baseY[vertex] ?? 0) + wingDeflectionMeters(wing.span[vertex] ?? 0, speed));
        }
        position.needsUpdate = true;
        wing.mesh.geometry.computeVertexNormals();
      }
      for (const rib of ribs) {
        pointRod(rib.rod,
          wingRibPoint(rib.side, rib.span, rib.from, speed),
          wingRibPoint(rib.side, rib.span, rib.to, speed));
      }
      for (const spar of sparSegments) {
        pointRod(spar.rod,
          wingSparPoint(spar.side, spar.from, speed),
          wingSparPoint(spar.side, spar.to, speed));
      }
      for (const foam of foamRibs) {
        foam.mesh.position.y = wingDeflectionMeters(foam.span, speed);
      }
      for (const stringer of stringers) {
        pointRod(stringer.rod,
          wingRibPoint(stringer.side, stringer.from, stringer.chordFraction, speed),
          wingRibPoint(stringer.side, stringer.to, stringer.chordFraction, speed));
      }
    }
    elevator.rotation.x = -Math.max(-0.35, Math.min(0.35, elevatorRadians));
    rudder.rotation.y = Math.max(-0.35, Math.min(0.35, rudderRadians));
  };
  setVisualState(null, 0, 0);
  return {
    root,
    setVisualState,
    dispose() {
      const geometries = new Set<BufferGeometry>();
      const materials = new Set<MeshLambertMaterial>();
      root.traverse((part) => {
        if (part instanceof Mesh) {
          geometries.add(part.geometry as BufferGeometry);
          materials.add(part.material as MeshLambertMaterial);
        }
      });
      for (const geometry of geometries) geometry.dispose();
      for (const material of materials) material.dispose();
      wingPattern.dispose();
    }
  };
}
