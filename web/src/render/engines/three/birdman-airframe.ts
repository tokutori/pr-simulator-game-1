import {
  BufferGeometry,
  CylinderGeometry,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshLambertMaterial,
  Vector3
} from "three";

const HALF_SPAN_METERS = 11.5;
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
  const spanSteps = shape.flex ? 28 : 10;
  const chordSteps = 16;
  const rowSize = chordSteps + 1;
  const surfaceSize = (spanSteps + 1) * rowSize;
  const vertices: number[] = [];
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
        const y = shape.baseY + shape.dihedral * distance +
          (upper ? profile.upperY : profile.lowerY) * chord;
        const z = leadingZ + (upper ? profile.upperX : profile.lowerX) * chord;
        vertices.push(shape.side * distance, y, z);
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

interface CockpitStation {
  readonly z: number;
  readonly centerY: number;
  readonly halfWidth: number;
  readonly height: number;
}

function cockpitLoft(stations: readonly CockpitStation[], upperHalf: boolean): BufferGeometry {
  const angularSteps = 16;
  const rowSize = angularSteps + 1;
  const vertices: number[] = [];
  const indices: number[] = [];
  for (const station of stations) {
    for (let angleIndex = 0; angleIndex <= angularSteps; angleIndex++) {
      const angle = angleIndex / angularSteps * Math.PI * (upperHalf ? 1 : 2);
      vertices.push(
        station.halfWidth * Math.cos(angle),
        station.centerY + station.height * Math.sin(angle),
        station.z
      );
    }
  }
  for (let section = 0; section < stations.length - 1; section++) {
    for (let angleIndex = 0; angleIndex < angularSteps; angleIndex++) {
      const a = section * rowSize + angleIndex;
      const b = a + rowSize;
      indices.push(a, b, a + 1, a + 1, b, b + 1);
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
  const spanFraction = span / HALF_SPAN_METERS;
  const chord = 1.6 + (0.55 - 1.6) * spanFraction;
  const profile = sectionProfile(chordFraction, 0.04, 0.4, 0.12);
  const meanY = (profile.upperY + profile.lowerY) * 0.5;
  return new Vector3(
    side * span,
    -0.73 + 0.025 * span + meanY * chord + wingDeflectionMeters(span, speed),
    -0.42 + 0.58 * spanFraction + chordFraction * chord
  );
}

export function createBirdmanAirframe(): BirdmanAirframe {
  const root = new Group();
  root.name = "birdman-airframe";
  const skin = new MeshLambertMaterial({ color: 0xe5e3d6, side: DoubleSide });
  const trailingFilm = new MeshLambertMaterial({
    color: 0xd5ddd9, side: DoubleSide, transparent: true, opacity: 0.22, depthWrite: false
  });
  const ribMaterial = new MeshLambertMaterial({ color: 0x84918d });
  const tailSkin = new MeshLambertMaterial({ color: 0xdddccc, side: DoubleSide });
  const carbon = new MeshLambertMaterial({ color: 0x303b3a });
  const fuselageSkin = new MeshLambertMaterial({ color: 0xd3d6cc, side: DoubleSide });
  const canopySkin = new MeshLambertMaterial({ color: 0x3e5760, side: DoubleSide });
  const wings: AirfoilHalf[] = [];
  const braces: { rod: Mesh; from: Vector3; to: Vector3; span: number }[] = [];
  const ribs: { rod: Mesh; side: -1 | 1; span: number; from: number; to: number }[] = [];
  for (const side of [-1, 1] as const) {
    const wingShape = {
      side, halfSpan: HALF_SPAN_METERS, rootChord: 1.6, tipChord: 0.55,
      leadingZ: -0.42, sweep: 0.58, baseY: -0.73, dihedral: 0.025,
      camber: 0.04, camberPosition: 0.4, thickness: 0.12,
      flex: true
    };
    const leadingWing = airfoilHalf({ ...wingShape, chordStart: 0, chordEnd: 0.58 }, skin);
    leadingWing.mesh.name = side === -1 ? "left-wing" : "right-wing";
    root.add(leadingWing.mesh);
    const film = airfoilHalf({ ...wingShape, chordStart: 0.58, chordEnd: 1 }, trailingFilm);
    film.mesh.name = side === -1 ? "left-trailing-film" : "right-trailing-film";
    root.add(film.mesh);
    wings.push(leadingWing, film);
    for (let ribIndex = 1; ribIndex <= 11; ribIndex++) {
      const span = ribIndex * 0.94;
      for (const [from, to] of [[0.58, 0.79], [0.79, 1]] as const) {
        const rod = addRod(root,
          wingRibPoint(side, span, from, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          wingRibPoint(side, span, to, REFERENCE_AIRSPEED_METERS_PER_SECOND),
          0.012, ribMaterial);
        rod.name = side === -1 ? "left-trailing-rib" : "right-trailing-rib";
        ribs.push({ rod, side, span, from, to });
      }
    }
    for (const [from, to, radius, span] of [
      [new Vector3(side * 0.28, -1.3, -0.7), new Vector3(side * 4.3, -0.73 + 4.3 * 0.025, 0.0), 0.023, 4.3],
      [new Vector3(side * 0.25, -1.25, -0.4), new Vector3(side * 5.8, -0.73 + 5.8 * 0.025, 0.65), 0.012, 5.8]
    ] as const) {
      braces.push({ rod: addRod(root, from, to, radius, carbon), from, to, span });
    }
  }
  addRod(root, new Vector3(0, -1.05, -1.8), new Vector3(0, -0.67, 5.15), 0.065, carbon);
  addRod(root, new Vector3(-0.7, -0.74, 0.15), new Vector3(0, -1.3, -0.65), 0.035, carbon);
  addRod(root, new Vector3(0.7, -0.74, 0.15), new Vector3(0, -1.3, -0.65), 0.035, carbon);
  const cockpitShell = new Mesh(cockpitLoft([
    { z: -2.35, centerY: -1.12, halfWidth: 0.015, height: 0.025 },
    { z: -1.85, centerY: -1.11, halfWidth: 0.12, height: 0.12 },
    { z: -1.35, centerY: -1.09, halfWidth: 0.25, height: 0.24 },
    { z: -0.80, centerY: -1.09, halfWidth: 0.34, height: 0.34 },
    { z: -0.15, centerY: -1.08, halfWidth: 0.33, height: 0.33 },
    { z: 0.50, centerY: -1.03, halfWidth: 0.22, height: 0.23 },
    { z: 1.20, centerY: -0.96, halfWidth: 0.08, height: 0.10 },
    { z: 1.60, centerY: -0.90, halfWidth: 0.015, height: 0.025 }
  ], false), fuselageSkin);
  cockpitShell.name = "enclosed-cockpit";
  root.add(cockpitShell);
  const canopy = new Mesh(cockpitLoft([
    { z: -1.47, centerY: -0.83, halfWidth: 0.015, height: 0.015 },
    { z: -1.18, centerY: -0.82, halfWidth: 0.20, height: 0.14 },
    { z: -0.77, centerY: -0.81, halfWidth: 0.27, height: 0.27 },
    { z: -0.25, centerY: -0.80, halfWidth: 0.27, height: 0.28 },
    { z: 0.25, centerY: -0.79, halfWidth: 0.21, height: 0.17 },
    { z: 0.58, centerY: -0.78, halfWidth: 0.015, height: 0.015 }
  ], true), canopySkin);
  canopy.name = "opaque-canopy";
  root.add(canopy);

  const tail = new Group();
  tail.position.set(0, -0.52, 4.5);
  root.add(tail);
  for (const side of [-1, 1] as const) {
    const basis = {
      side, halfSpan: 2.1, rootChord: 0.82, tipChord: 0.72,
      leadingZ: 0, sweep: 0.08, baseY: 0, dihedral: 0.02,
      camber: 0, camberPosition: 0.4, thickness: 0.09, flex: false
    };
    const stabilizer = airfoilHalf({ ...basis, chordStart: 0, chordEnd: 0.7 }, tailSkin);
    tail.add(stabilizer.mesh);
  }
  const elevator = new Group();
  elevator.name = "elevator";
  elevator.position.set(0, 0, 0.574);
  tail.add(elevator);
  for (const side of [-1, 1] as const) {
    const half = airfoilHalf({
      side, halfSpan: 2.1, rootChord: 0.82, tipChord: 0.72,
      leadingZ: -0.574, sweep: 0.08, baseY: 0, dihedral: 0.02,
      camber: 0, camberPosition: 0.4, thickness: 0.09,
      chordStart: 0.7, chordEnd: 1, flex: false
    }, trailingFilm);
    half.mesh.name = side === -1 ? "left-elevator-film" : "right-elevator-film";
    elevator.add(half.mesh);
    for (const span of [0.35, 0.90, 1.45, 2.0]) {
      const chord = 0.82 + (0.72 - 0.82) * span / 2.1;
      const sweep = 0.08 * span / 2.1;
      const y = 0.02 * span;
      const rod = addRod(elevator,
        new Vector3(side * span, y, -0.574 + sweep + 0.7 * chord),
        new Vector3(side * span, y, -0.574 + sweep + chord),
        0.008, ribMaterial);
      rod.name = "elevator-rib";
    }
  }
  const rudder = new Group();
  rudder.name = "rudder";
  tail.add(rudder);
  const finFront = new Mesh(finGeometry([
    [0, 0.02], [1.33, 0.25], [1.52, 0.36], [0, 0.39]
  ], 0.045), tailSkin);
  finFront.name = "rudder-front";
  rudder.add(finFront);
  const finFilm = new Mesh(finGeometry([
    [0, 0.39], [1.52, 0.36], [1.52, 0.53], [0, 0.62]
  ], 0.036), trailingFilm);
  finFilm.name = "rudder-trailing-film";
  rudder.add(finFilm);
  for (const y of [0.25, 0.60, 0.95, 1.30]) {
    const fraction = y / 1.52;
    const rod = addRod(rudder,
      new Vector3(0, y, 0.39 - 0.03 * fraction),
      new Vector3(0, y, 0.62 - 0.09 * fraction),
      0.009, ribMaterial);
    rod.name = "rudder-rib";
  }

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
      for (const brace of braces) {
        const bentEnd = brace.to.clone();
        bentEnd.y += wingDeflectionMeters(brace.span, speed);
        pointRod(brace.rod, brace.from, bentEnd);
      }
      for (const rib of ribs) {
        pointRod(rib.rod,
          wingRibPoint(rib.side, rib.span, rib.from, speed),
          wingRibPoint(rib.side, rib.span, rib.to, speed));
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
    }
  };
}
