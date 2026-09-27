import {
  CanvasTexture,
  Color,
  DoubleSide,
  LinearFilter,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SRGBColorSpace,
  WebGLRenderer
} from "three";
import type { Object3D } from "three";
import type { BackendFrame, RendererAdapter, ViewportSize } from "../../contracts/runtime.js";
import type { Pose } from "../../contracts/math.js";

export function createThreeRenderer(canvas: HTMLCanvasElement, panelCanvas: HTMLCanvasElement): RendererAdapter {
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setClearColor(0x8aadb0, 1);

  const scene = new Scene();
  scene.background = new Color(0x8aadb0);

  const water = new Mesh(
    new PlaneGeometry(600, 600),
    new MeshBasicMaterial({ color: 0x527e82 })
  );
  water.rotation.x = -Math.PI / 2;
  water.position.y = -2.4;
  scene.add(water);

  const panelTexture = new CanvasTexture(panelCanvas);
  panelTexture.colorSpace = SRGBColorSpace;
  panelTexture.minFilter = LinearFilter;
  panelTexture.generateMipmaps = false;
  const panelMaterial = new MeshBasicMaterial({ map: panelTexture, side: DoubleSide });
  const panelGeometry = new PlaneGeometry(2.4, 1.8);
  const panelMesh = new Mesh(panelGeometry, panelMaterial);
  panelMesh.visible = false;
  scene.add(panelMesh);

  const camera = new PerspectiveCamera(60, 1, 0.05, 2000);
  camera.position.set(0, 0, 0);
  let disposed = false;
  let loopRunning = false;
  let panelRevision = -1;
  let width = 0;
  let height = 0;
  let pixelRatio = 0;

  return {
    startLoop(callback) {
      ensureActive(disposed);
      if (loopRunning) throw new Error("Three.js frame loop is already active");
      loopRunning = true;
      renderer.setAnimationLoop((timestamp) => { callback(timestamp); });
    },
    stopLoop() {
      if (disposed || !loopRunning) return;
      renderer.setAnimationLoop(null);
      loopRunning = false;
    },
    render(frame: BackendFrame) {
      ensureActive(disposed);
      resizeIfNeeded(frame.viewport);
      setPose(camera, frame.cameraPose);
      setPose(panelMesh, frame.menuPose);
      panelMesh.visible = frame.panelVisible;
      if (frame.panelRevision !== panelRevision) {
        panelTexture.needsUpdate = true;
        panelRevision = frame.panelRevision;
      }
      renderer.render(scene, camera);
    },
    resize(viewport: ViewportSize) {
      ensureActive(disposed);
      resizeIfNeeded(viewport);
    },
    dispose() {
      if (disposed) return;
      if (loopRunning) renderer.setAnimationLoop(null);
      loopRunning = false;
      panelTexture.dispose();
      panelGeometry.dispose();
      panelMaterial.dispose();
      water.geometry.dispose();
      water.material.dispose();
      renderer.dispose();
      disposed = true;
    }
  };

  function resizeIfNeeded(viewport: ViewportSize): void {
    if (viewport.x === width && viewport.y === height && viewport.pixelRatio === pixelRatio) return;
    width = viewport.x;
    height = viewport.y;
    pixelRatio = viewport.pixelRatio;
    renderer.setPixelRatio(pixelRatio);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  }
}

function setPose(object: Object3D, pose: Pose): void {
  object.position.set(pose.position.x, pose.position.y, pose.position.z);
  object.quaternion.set(pose.orientation.x, pose.orientation.y, pose.orientation.z, pose.orientation.w);
}

function ensureActive(disposed: boolean): void {
  if (disposed) throw new Error("Three.js renderer has been disposed");
}
