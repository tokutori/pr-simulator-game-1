import {
  AmbientLight, Color, DirectionalLight, GridHelper, OrthographicCamera,
  Scene, WebGLRenderer
} from "three";
import { createBirdmanAirframe } from "./birdman-airframe.js";

type ViewName = "top" | "side" | "front";
interface ViewSpec {
  readonly name: ViewName;
  readonly position: readonly [number, number, number];
  readonly target: readonly [number, number, number];
  readonly up: readonly [number, number, number];
  readonly extent: number;
}

const specs: readonly ViewSpec[] = [
  { name: "top", position: [0, 20, 0], target: [0, 0, 0], up: [0, 0, -1], extent: 6.5 },
  { name: "side", position: [20, 0, 1.85], target: [0, 0, 1.85], up: [0, 1, 0], extent: 2.5 },
  { name: "front", position: [0, 0, -20], target: [0, 0, 0], up: [0, 1, 0], extent: 6.5 }
];

function addScaleGrid(scene: Scene, name: ViewName): void {
  const grid = name === "side"
    ? new GridHelper(10, 20, 0x59626a, 0x454d53)
    : new GridHelper(24, 24, 0x59626a, 0x454d53);
  if (name === "top") grid.position.y = -3;
  if (name === "side") {
    grid.rotation.z = Math.PI / 2;
    grid.position.x = -3;
    grid.position.z = 1.6;
  }
  if (name === "front") {
    grid.rotation.x = Math.PI / 2;
    grid.position.z = 6;
  }
  scene.add(grid);
}

for (const spec of specs) {
  const panel = document.querySelector<HTMLElement>(`[data-view="${spec.name}"]`);
  if (panel === null) throw new Error(`Missing ${spec.name} orthographic panel`);
  const scene = new Scene();
  scene.background = new Color(0x363c42);
  scene.add(createBirdmanAirframe().root);
  addScaleGrid(scene, spec.name);
  scene.add(new AmbientLight(0xffffff, 2));
  const sun = new DirectionalLight(0xffffff, 2);
  sun.position.set(-4, 12, -8);
  scene.add(sun);
  const renderer = new WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  panel.append(renderer.domElement);
  const camera = new OrthographicCamera(-spec.extent, spec.extent, spec.extent, -spec.extent, 0.1, 100);
  camera.position.set(...spec.position);
  camera.up.set(...spec.up);
  camera.lookAt(...spec.target);
  const render = (): void => {
    const { clientWidth: width, clientHeight: height } = panel;
    if (width === 0 || height === 0) return;
    renderer.setSize(width, height);
    const aspect = width / height;
    const halfWidthNeeded = spec.name === "side" ? 3.5 : 11.5;
    const halfHeight = Math.max(spec.extent, halfWidthNeeded / aspect);
    camera.left = -halfHeight * aspect;
    camera.right = halfHeight * aspect;
    camera.top = halfHeight;
    camera.bottom = -halfHeight;
    camera.updateProjectionMatrix();
    renderer.render(scene, camera);
  };
  new ResizeObserver(render).observe(panel);
  requestAnimationFrame(render);
}
