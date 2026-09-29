import {
  BufferGeometry,
  CylinderGeometry,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshLambertMaterial,
  SphereGeometry,
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

export function createBirdmanAirframe(): BirdmanAirframe {
  const root = new Group();
  root.name = "birdman-airframe";
  const skin = new MeshLambertMaterial({ color: 0xe5e3d6, side: DoubleSide });
  const tailSkin = new MeshLambertMaterial({ color: 0xdddccc, side: DoubleSide });
  const carbon = new MeshLambertMaterial({ color: 0x303b3a });
  const cockpit = new MeshLambertMaterial({ color: 0x51605b });
  const helmet = new MeshLambertMaterial({ color: 0xe0c777 });
  const wings: AirfoilHalf[] = [];
  const braces: { rod: Mesh; from: Vector3; to: Vector3; span: number }[] = [];
  for (const side of [-1, 1] as const) {
    const wing = airfoilHalf({
      side, halfSpan: HALF_SPAN_METERS, rootChord: 1.6, tipChord: 0.55,
      leadingZ: -0.42, sweep: 0.58, baseY: -0.73, dihedral: 0.025,
      camber: 0.04, camberPosition: 0.4, thickness: 0.12,
      chordStart: 0, chordEnd: 1, flex: true
    }, skin);
    wing.mesh.name = side === -1 ? "left-wing" : "right-wing";
    root.add(wing.mesh);
    wings.push(wing);
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
  const pod = new Mesh(new SphereGeometry(1, 16, 12), cockpit);
  pod.position.set(0, -1.37, -0.95);
  pod.scale.set(0.24, 0.15, 1.05);
  root.add(pod);
  const pilotTorso = new Mesh(new SphereGeometry(1, 12, 10), carbon);
  pilotTorso.position.set(0, -1.13, -0.47);
  pilotTorso.scale.set(0.2, 0.33, 0.22);
  root.add(pilotTorso);
  const pilotHead = new Mesh(new SphereGeometry(0.17, 12, 10), helmet);
  pilotHead.position.set(0, -0.68, -0.59);
  root.add(pilotHead);

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
    }, tailSkin);
    elevator.add(half.mesh);
  }
  const fin = new Mesh(finGeometry([
    [0, 0.02], [1.33, 0.25], [1.52, 0.53], [0, 0.62]
  ], 0.045), tailSkin);
  fin.name = "vertical-stabilizer";
  tail.add(fin);
  const rudder = new Group();
  rudder.name = "rudder";
  rudder.position.z = 0.62;
  tail.add(rudder);
  rudder.add(new Mesh(finGeometry([
    [0, 0], [1.52, -0.09], [1.33, 0.24], [0, 0.36]
  ], 0.035), tailSkin));

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
