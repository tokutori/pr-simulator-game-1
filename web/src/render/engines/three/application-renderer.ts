import { AmbientLight, BufferGeometry, CanvasTexture, CircleGeometry, Color, DirectionalLight, DoubleSide, Fog, FrontSide,
  Group, LinearFilter, Matrix4, Mesh, MeshBasicMaterial, MeshLambertMaterial, PerspectiveCamera, PlaneGeometry,
  Scene, ShaderMaterial, SRGBColorSpace, UniformsLib, UniformsUtils, Vector2, Vector4, WebGLRenderer, WebGLRenderTarget } from "three";
import type { Material, Object3D } from "three";
import type { ApplicationDrawingOutput, ApplicationGraphicsTarget, ApplicationInput, ApplicationMeshAsset,
  ApplicationMeshAttachment, ApplicationRequest, ApplicationRendererObservation, DisplayRequest, Pose, XrSurfaceStyle } from "../../../generated/application-contracts.js";
import { ThreeApplicationBuffers } from "./application-buffers.js";
import { GpuWorkTimer } from "./gpu-work-timer.js";
import { browserSurfaceContext } from "../../../presentation/browser-canvas.js";
import { paintSurface } from "../../../presentation/surface-canvas.js";
import { lakeFilteredWaterVertexShader, lakeWaterFragmentShader, lakeWaterVertexShader } from "./water-shaders.js";

type PrepareRequest = Extract<ApplicationRequest, { kind: "prepare_resources" }>;
type ResourceRequest = Extract<ApplicationRequest, { kind: "commit_resources" | "cancel_resources" | "release_resources" }>;

function submitSun(sun: DirectionalLight, light: ApplicationDrawingOutput["scene"]["lighting"], intensity: number): void {
  sun.intensity = light == null ? 0 : intensity;
  sun.castShadow = light?.shadow_enabled === true;
  if (light == null) return;
  sun.position.fromArray(light.sun_world_position);
  sun.target.position.fromArray(light.sun_target_world_position);
  sun.shadow.normalBias = light.normal_bias;
  sun.shadow.bias = light.depth_bias;
  sun.shadow.radius = 5;
  const [left, right, bottom, top, near, far] = light.shadow_bounds;
  Object.assign(sun.shadow.camera, { left, right, bottom, top, near, far });
  sun.shadow.camera.updateProjectionMatrix();
}

interface PreparedResources {
  readonly request: PrepareRequest;
  readonly scene: Scene;
  readonly aircraft: Group;
  readonly attachments: Readonly<Record<ApplicationMeshAttachment, Group>>;
  readonly water: Group;
  readonly water_materials: readonly ShaderMaterial[];
  readonly water_geometries: readonly BufferGeometry[];
  readonly sun: DirectionalLight;
  readonly reflection: WebGLRenderTarget;
  readonly image_leases: readonly number[];
}

interface XrHandle {
  readonly session: XRSession;
  reference: XRReferenceSpace | null;
  detach: () => void;
  ended: boolean;
}

export class ThreeApplicationRenderer {
  private readonly renderer: WebGLRenderer;
  private readonly buffers: ThreeApplicationBuffers;
  private readonly timer: GpuWorkTimer;
  private readonly resources = new Map<number, PreparedResources>();
  private readonly preparations = new Map<number, { cancelled: boolean }>();
  private readonly base_geometry = new Map<number, BufferGeometry>();
  private readonly range_geometry = new Map<number, readonly BufferGeometry[]>();
  private readonly materials = new Map<string, Material>();
  private readonly sources = new WeakMap<XRInputSource, number>();
  private readonly gamepad_generations = new Map<number, number>();
  private readonly sessions = new Map<number, XrHandle>();
  private readonly display_handles = new Set<number>();
  private readonly camera = new PerspectiveCamera();
  private readonly reflection_camera = new PerspectiveCamera();
  private readonly menu_canvas: HTMLCanvasElement;
  private readonly head_canvas: HTMLCanvasElement;
  private menu_mesh: Mesh<PlaneGeometry, MeshBasicMaterial> | null = null;
  private head_mesh: Mesh<PlaneGeometry, MeshBasicMaterial> | null = null;
  private cursor_geometry: CircleGeometry | null = null;
  private readonly cursor_meshes: Mesh<CircleGeometry, MeshBasicMaterial>[] = [];
  private manifest: PrepareRequest["assets"]["scene"] = null;
  private active: PreparedResources | null = null;
  private drawing: ApplicationDrawingOutput | null = null;
  private display_id: number | null = null;
  private presented_id: number | null = null;
  private next_source = 0;
  private disposed = false;
  private quality: ApplicationGraphicsTarget["quality"] = { scale: "100", shadow: "full", water_quality: "high" };

  constructor(private readonly canvas: HTMLCanvasElement, private readonly id: number,
    bytes: (id: number) => Uint8Array, private readonly notify: (input: ApplicationInput) => void,
    private readonly platformChanged: () => void) {
    this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.shadowMap.enabled = true;
    this.renderer.xr.cameraAutoUpdate = false;
    this.reflection_camera.layers.set(1);
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(window.innerWidth, window.innerHeight, false);
    this.buffers = new ThreeApplicationBuffers(bytes);
    this.timer = new GpuWorkTimer(this.renderer.getContext() as WebGL2RenderingContext);
    this.menu_canvas = canvas.ownerDocument.createElement("canvas");
    this.head_canvas = canvas.ownerDocument.createElement("canvas");
    canvas.addEventListener("webglcontextlost", this.contextLost);
    canvas.addEventListener("pointermove", this.pointerMove);
    canvas.addEventListener("contextmenu", this.contextMenu);
    window.addEventListener("gamepadconnected", this.gamepadConnection);
    window.addEventListener("gamepaddisconnected", this.gamepadConnection);
    this.renderer.xr.addEventListener("sessionstart", this.platformChanged);
    this.renderer.xr.addEventListener("sessionend", this.platformChanged);
    this.renderer.setAnimationLoop(this.frame);
  }

  observation(): ApplicationRendererObservation {
    const size = this.renderer.getDrawingBufferSize(new Vector2());
    const unsupported = this.timer.availability();
    const layer = this.renderer.xr.getSession()?.renderState.baseLayer;
    return { resource_id: this.id, initial_quality: this.quality,
      extent_px: [size.x, size.y], screen_extent_px: [Math.round(window.innerWidth * window.devicePixelRatio), Math.round(window.innerHeight * window.devicePixelRatio)],
      xr_extent_px: layer == null ? null : [layer.framebufferWidth, layer.framebufferHeight],
      gpu_timing: unsupported === null ? { kind: "available" } : { kind: "unsupported", cause: unsupported.cause } };
  }

  owns(request: ResourceRequest): boolean {
    return request.kind === "cancel_resources" ? this.preparations.has(request.request_id) : this.resources.has(request.resource_id);
  }

  ownsDisplay(id: number): boolean {
    return this.sessions.has(id) || this.display_handles.has(id);
  }

  prepare(request: PrepareRequest): void {
    const preparation = { cancelled: false };
    this.preparations.set(request.request_id, preparation);
    let candidate: PreparedResources;
    try { candidate = this.createResources(request); }
    catch (cause) {
      this.preparations.delete(request.request_id);
      this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "failed", cause: errorMessage(cause) } });
      return;
    }
    void this.compile(candidate).then(() => {
      this.preparations.delete(request.request_id);
      if (preparation.cancelled) {
        let failure: string | null = null;
        try { this.releaseCandidate(candidate); } catch (cause) { failure = errorMessage(cause); }
        this.notify({ kind: "resources", request_id: request.request_id, observation: failure === null
          ? { kind: "cancelled" } : { kind: "failed", cause: failure } });
      } else {
        this.resources.set(request.request_id, candidate);
        this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "prepared", resource_id: request.request_id } });
      }
    }, (cause: unknown) => {
      this.preparations.delete(request.request_id);
      let failure = cause;
      try { this.releaseCandidate(candidate); } catch (cleanup) { failure = new AggregateError([cause, cleanup], "GPU preparation cleanup failed"); }
      this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "failed", cause: errorMessage(failure) } });
    });
  }

  private async compile(candidate: PreparedResources): Promise<void> {
    const surfaces = [this.menu_mesh, this.head_mesh, this.cursor_meshes[0]].flatMap(mesh => {
      if (mesh == null) return [];
      const surface = mesh.clone();
      surface.visible = true;
      return [surface];
    });
    const xr = this.renderer.xr.enabled;
    const target = this.renderer.getRenderTarget();
    const background = candidate.scene.background;
    const pending: Promise<unknown>[] = [];
    const failures: unknown[] = [];
    try {
      candidate.scene.add(...surfaces);
      this.renderer.xr.enabled = false;
      pending.push(this.renderer.compileAsync(candidate.scene, this.camera));
      candidate.scene.background = null;
      this.renderer.setRenderTarget(candidate.reflection);
      pending.push(this.renderer.compileAsync(candidate.scene, this.reflection_camera));
    } catch (cause) { failures.push(cause); }
    finally {
      for (const restore of [() => { candidate.scene.remove(...surfaces); },
        () => { candidate.scene.background = background; }, () => { this.renderer.xr.enabled = xr; },
        () => { this.renderer.setRenderTarget(target); }]) {
        try { restore(); } catch (cause) { failures.push(cause); }
      }
    }
    for (const result of await Promise.allSettled(pending)) if (result.status === "rejected") failures.push(result.reason);
    if (failures.length > 0) throw new AggregateError(failures, "GPU compilation failed");
  }

  execute(request: ResourceRequest): void {
    if (request.kind === "cancel_resources") {
      const preparation = this.preparations.get(request.request_id);
      if (preparation !== undefined) preparation.cancelled = true;
      return;
    }
    const candidate = this.resources.get(request.resource_id);
    if (candidate === undefined) throw new Error("Requested prepared GPU handle does not exist");
    if (request.kind === "release_resources") {
      this.resources.delete(request.resource_id);
      if (this.active === candidate) this.active = null;
      let failure: string | null = null;
      try { this.releaseCandidate(candidate); } catch (cause) { failure = errorMessage(cause); }
      this.notify({ kind: "resources_released", resource_id: request.resource_id, failure });
      return;
    }
    const target = candidate.request.graphics;
    const previousRatio = this.renderer.getPixelRatio();
    const previousSize = this.renderer.getSize(new Vector2());
    let extent: [number, number];
    try {
      this.renderer.setPixelRatio(1);
      this.renderer.setSize(target.extent_px[0], target.extent_px[1], false);
      candidate.sun.shadow.mapSize.fromArray(target.shadow_extent_px);
      const size = this.renderer.getDrawingBufferSize(new Vector2());
      extent = [size.x, size.y];
    } catch (cause) {
      try {
        this.renderer.setPixelRatio(previousRatio);
        this.renderer.setSize(previousSize.x, previousSize.y, false);
      } catch (rollback) {
        this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "binding_lost", resource_id: request.resource_id,
          cause: errorMessage(new AggregateError([cause, rollback], "GPU commit rollback failed")) } });
        return;
      }
      this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "failed", cause: errorMessage(cause) } });
      return;
    }
    this.quality = target.quality;
    this.active = candidate;
    this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "committed", resource_id: request.resource_id,
      actual: { applied: this.quality, extent_px: extent } } });
  }

  private createResources(request: PrepareRequest): PreparedResources {
    this.menu_mesh ??= this.surface(this.menu_canvas);
    this.head_mesh ??= this.surface(this.head_canvas);
    if (this.cursor_meshes.length === 0) {
      this.cursor_geometry = new CircleGeometry(0.5, 32);
      const cursor = new Mesh(this.cursor_geometry, new MeshBasicMaterial({ side: DoubleSide, transparent: true, depthWrite: false }));
      cursor.visible = false;
      this.cursor_meshes.push(cursor);
    }
    const assets = request.assets;
    if (assets.scene != null) this.manifest = assets.scene;
    if (this.manifest == null) throw new Error("Prepared scene manifest has not been submitted to this graphics context");
    const cleanup: (() => void)[] = [];
    try {
    const scene = new Scene();
    const lighting = assets.lighting;
    scene.background = this.buffers.image(assets.sky, true);
    cleanup.push(() => { this.buffers.releaseImages([assets.sky.buffer_id]); });
    scene.fog = new Fog(lighting.haze_color_srgb, lighting.haze_range_meters[0], lighting.haze_range_meters[1]);
    const images = [assets.sky.buffer_id];
    for (const texture of this.manifest.wing_textures) this.buffers.wingTexture(texture);
    const sun = new DirectionalLight(lighting.sunlight_color_srgb, lighting.sunlight_irradiance);
    submitSun(sun, lighting.initial_shadow, lighting.sunlight_irradiance);
    sun.shadow.mapSize.fromArray(request.graphics.shadow_extent_px);
    const ambient = new AmbientLight(lighting.ambient_color_srgb, lighting.ambient_irradiance);
    ambient.layers.enable(1);
    sun.layers.enable(1);
    scene.add(ambient, sun, sun.target);
    const aircraft = new Group();
    const attachments = { world: new Group(), aircraft, horizontal_tail: new Group(), vertical_tail: new Group() };
    aircraft.add(attachments.horizontal_tail, attachments.vertical_tail);
    scene.add(attachments.world, aircraft);
    for (const asset of this.manifest.meshes) {
      let ranges = this.range_geometry.get(asset.mesh_id);
      if (ranges === undefined) {
        const base = this.buffers.geometry(asset);
        this.base_geometry.set(asset.mesh_id, base);
        ranges = asset.materials.map(material => {
          const geometry = new BufferGeometry();
          geometry.setIndex(base.getIndex());
          for (const [name, attribute] of Object.entries(base.attributes)) geometry.setAttribute(name, attribute);
          geometry.setDrawRange(material.start, material.count);
          return geometry;
        });
        this.range_geometry.set(asset.mesh_id, ranges);
      }
      asset.materials.forEach((source, index) => {
        const geometry = ranges[index];
        if (geometry === undefined) throw new Error("Incomplete material range geometry");
        const mesh = new Mesh(geometry, this.material(asset, index));
        mesh.name = asset.name;
        mesh.castShadow = source.cast_shadow;
        mesh.receiveShadow = source.receive_shadow;
        mesh.frustumCulled = asset.frustum_culled;
        if (asset.attachment !== "world") mesh.layers.enable(1);
        setPose(mesh, asset.local_pose);
        attachments[asset.attachment].add(mesh);
      });
    }
    const [near, far] = assets.water.detail;
    const nearTexture = this.buffers.image(near.image, false);
    images.push(near.image.buffer_id);
    cleanup.push(() => { this.buffers.releaseImages([near.image.buffer_id]); });
    const farTexture = this.buffers.image(far.image, false);
    images.push(far.image.buffer_id);
    cleanup.push(() => { this.buffers.releaseImages([far.image.buffer_id]); });
    const reflection = new WebGLRenderTarget(request.graphics.reflection_extent_px[0], request.graphics.reflection_extent_px[1], { minFilter: LinearFilter, magFilter: LinearFilter });
    cleanup.push(() => { reflection.dispose(); });
    reflection.texture.colorSpace = SRGBColorSpace;
    const projection = assets.water.projection;
    const uniforms = {
      ...UniformsUtils.clone(UniformsLib.lights), uTimeSeconds: { value: 0 },
      uWindSpeed: { value: assets.water.wind_speed_meters_per_second },
      uWindVelocity: { value: new Vector4(...assets.water.wind_velocity_render, 0, 0) },
      uDetailScale: { value: assets.water.detail_amplitude_scale }, uDetailNear: { value: nearTexture }, uDetailFar: { value: farTexture },
      uDetailExtents: { value: new Vector4(near.extent_meters, far.extent_meters, 0, 0) },
      uSkyTexture: { value: scene.background },
      uWaveKAmplitude: { value: projection.waveKAmplitude.map(wave => new Vector4(...wave)) },
      uWaveOmegaPhase: { value: projection.waveOmegaPhase.map(wave => new Vector4(...wave)) },
      uWaveCount: { value: projection.waveCount }, uChoppiness: { value: projection.choppiness },
      uGridSpacingGradient: { value: projection.spacing.gradientMagnitude }, uVisualWaveHeight: { value: projection.visualWaveHeight },
      uWaterDark: { value: new Color(assets.water.palette_srgb[0]) }, uWaterMid: { value: new Color(assets.water.palette_srgb[1]) }, uWaterLight: { value: new Color(assets.water.palette_srgb[2]) },
      uHazeColor: { value: new Color(lighting.haze_color_srgb) }, uHazeRange: { value: new Vector4(...lighting.haze_range_meters, 0, 0) },
      uSunDirection: { value: new Vector4(...lighting.sunlight_direction) }, uReflectionTexture: { value: reflection.texture },
      uReflectionMatrix: { value: new Matrix4() }, uReflectionEnabled: { value: 0 }, uWaterBackingClip: { value: 0 },
      uWaterBackingCenter: { value: new Vector4() }, uWaterBackingInnerExtent: { value: assets.water.geometry.backing_inner_extent_meters }
    };
    const material = new ShaderMaterial({ lights: true, uniforms, vertexShader: lakeWaterVertexShader, fragmentShader: lakeWaterFragmentShader });
    cleanup.push(() => { material.dispose(); });
    const backing = new ShaderMaterial({ lights: true, uniforms: { ...uniforms, uWaterBackingClip: { value: 1 },
      uReflectionEnabled: { value: 0 } }, vertexShader: lakeFilteredWaterVertexShader, fragmentShader: lakeWaterFragmentShader,
      side: DoubleSide, depthWrite: false });
    cleanup.push(() => { backing.dispose(); });
    const water = new Group();
    const waterGeometries = [...assets.water.geometry.near, assets.water.geometry.far].map((chunk, index) => {
      const geometry = this.buffers.waterGeometry(chunk);
      cleanup.push(() => { geometry.dispose(); });
      const mesh = new Mesh(geometry, index < assets.water.geometry.near.length ? material : backing);
      mesh.frustumCulled = chunk.frustum_culled;
      mesh.receiveShadow = true;
      if (index === assets.water.geometry.near.length) mesh.renderOrder = -1;
      water.add(mesh);
      return geometry;
    });
    scene.add(water);
    for (const texture of [scene.background, nearTexture, farTexture]) this.renderer.initTexture(texture);
    for (const mesh of [this.menu_mesh, this.head_mesh]) if (mesh.material.map !== null) this.renderer.initTexture(mesh.material.map);
    this.renderer.initRenderTarget(reflection);
    return { request, scene, aircraft, attachments, water, water_materials: [material, backing], sun, reflection, image_leases: images, water_geometries: waterGeometries };
    } catch (cause) {
      const failures = [cause];
      for (const release of cleanup.reverse()) {
        try { release(); } catch (failure) { failures.push(failure); }
      }
      throw new AggregateError(failures, "GPU resource creation failed", { cause });
    }
  }

  private material(asset: ApplicationMeshAsset, index: number): Material {
    const source = asset.materials[index];
    if (source === undefined) throw new Error("Missing shared material range");
    const key = JSON.stringify([source, asset.color != null]);
    let material = this.materials.get(key);
    if (material !== undefined) return material;
    const texture = this.manifest?.wing_textures.find(image => image.buffer.buffer_id === source.texture_buffer_id);
    const values = { color: source.color_srgb, opacity: source.opacity, transparent: source.opacity < 1,
      depthWrite: source.opacity >= 1, side: source.double_sided ? DoubleSide : FrontSide,
      vertexColors: asset.color != null, map: texture === undefined ? null : this.buffers.wingTexture(texture) };
    material = source.lit ? new MeshLambertMaterial(values) : new MeshBasicMaterial(values);
    this.materials.set(key, material);
    return material;
  }

  private releaseCandidate(candidate: PreparedResources): void {
    const resources = [...candidate.water_geometries, ...candidate.water_materials, candidate.reflection];
    const failures: unknown[] = [];
    for (const resource of resources) {
      try { resource.dispose(); } catch (cause) { failures.push(cause); }
    }
    try { this.buffers.releaseImages(candidate.image_leases); } catch (cause) { failures.push(cause); }
    if (failures.length > 0) throw new AggregateError(failures, "Prepared scene cleanup failed");
  }

  private surface(canvas: HTMLCanvasElement): Mesh<PlaneGeometry, MeshBasicMaterial> {
    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;
    texture.minFilter = LinearFilter;
    texture.generateMipmaps = false;
    const mesh = new Mesh(new PlaneGeometry(1, 1), new MeshBasicMaterial({ map: texture, side: DoubleSide, transparent: true, depthWrite: false }));
    mesh.visible = false;
    mesh.renderOrder = 2000;
    return mesh;
  }

  apply(drawing: ApplicationDrawingOutput): void {
    this.drawing = drawing;
    const resource = drawing.graphics.resource;
    this.canvas.hidden = !drawing.presentation.scene_visible || resource?.renderer_resource_id !== this.id;
    if (resource?.renderer_resource_id !== this.id || this.active === null) return;
    const geometry = drawing.scene.geometry;
    if (geometry != null) {
      for (const update of geometry.updates) {
        const base = this.base_geometry.get(update.mesh_id);
        if (base === undefined) throw new Error("Dynamic upload has no prepared static mesh");
        this.buffers.update(base, update);
        for (const range of this.range_geometry.get(update.mesh_id) ?? []) {
          range.setAttribute("position", base.getAttribute("position"));
          range.setAttribute("normal", base.getAttribute("normal"));
        }
      }
    }
    for (const candidate of this.resources.values()) {
      setPose(candidate.aircraft, drawing.scene.world_from_aircraft);
      candidate.aircraft.visible = geometry?.aircraft_visible === true;
      candidate.attachments.world.visible = drawing.scene.venue_visible;
      for (const part of geometry?.part_poses ?? []) setPose(candidate.attachments[part.attachment], part.pose);
      setPose(candidate.water, drawing.scene.water_world_from_surface);
      submitSun(candidate.sun, candidate === this.active ? drawing.scene.lighting : candidate.request.assets.lighting.initial_shadow,
        candidate.request.assets.lighting.sunlight_irradiance);
      for (const material of candidate.water_materials) {
        uniform(material, "uTimeSeconds", drawing.scene.water_time_seconds);
        uniform(material, "uWaterBackingCenter", new Vector4(...drawing.scene.water_clip_center, 0, 0));
      }
    }
  }

  display(request: DisplayRequest): void {
    if (request.kind === "permission") {
      if (request.mode === "hmd") {
        let permission: Promise<XRSession>;
        try {
          if (navigator.xr === undefined) throw new Error("WebXR is unavailable");
          permission = navigator.xr.requestSession("immersive-vr", { requiredFeatures: ["local"] });
        } catch (cause) {
          this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
          return;
        }
        void permission.then(session => {
          const handle: XrHandle = { session, reference: null, ended: false, detach: () => {
            session.removeEventListener("end", ended);
            session.removeEventListener("visibilitychange", visibility);
          } };
          const ended = (): void => {
            handle.ended = true;
            queueMicrotask(() => { this.notify({ kind: "display_ended", resource_id: request.request_id }); });
          };
          const visibility = (): void => { this.notify({ kind: "xr_session_state", resource_id: request.request_id,
            visibility: xrVisibility(session), input_focused: session.visibilityState === "visible" }); };
          session.addEventListener("end", ended);
          session.addEventListener("visibilitychange", visibility);
          this.sessions.set(request.request_id, handle);
          this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "permission_granted", resource_id: request.request_id } });
        }, (cause: unknown) => {
          this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
        });
      } else if (request.mode === "phone_vr") {
        const orientation = DeviceOrientationEvent as typeof DeviceOrientationEvent & { requestPermission?: (absolute: boolean) => Promise<PermissionState> };
        let permission: Promise<PermissionState>;
        try { permission = orientation.requestPermission?.(true) ?? Promise.resolve("granted"); }
        catch (cause) {
          this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
          return;
        }
        void permission.then(value => {
          this.notify({ kind: "display_completed", request_id: request.request_id,
            completion: value === "granted" ? { kind: "permission_granted" } : { kind: "failed", cause: "Sensor permission denied" } });
        }, (cause: unknown) => {
          this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
        });
      } else this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "permission_granted" } });
      return;
    }
    if (request.kind === "release") {
      void this.releaseDisplay(request.resource_id).then(() => {
        this.notify({ kind: "display_released", resource_id: request.resource_id, failure: null });
      }, (cause: unknown) => {
        this.notify({ kind: "display_released", resource_id: request.resource_id, failure: errorMessage(cause) });
      });
      return;
    }
    if (request.kind === "stop") {
      const stop = this.stopDisplay(request.resource_id ?? null);
      void stop.then(() => {
        this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "stopped" } });
      }, (cause: unknown) => {
        this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
      });
      return;
    }
    if (request.mode === "hmd") {
      const id = request.permission_resource_id;
      const handle = id == null ? undefined : this.sessions.get(id);
      if (handle === undefined || id == null) {
        this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: "Acquired XR session is unavailable" } });
        return;
      }
      this.renderer.xr.enabled = true;
      this.renderer.xr.setReferenceSpaceType("local");
      const start = async (): Promise<void> => {
        await this.renderer.xr.setSession(handle.session);
        const reference = this.renderer.xr.getReferenceSpace();
        if (reference === null) throw new Error("XR reference space is unavailable");
        handle.reference = reference;
        const reset = (event: XRReferenceSpaceEvent): void => {
          this.notify({ kind: "xr_reference_reset", resource_id: id, previous_from_new: event.transform == null ? null : xrPose(event.transform) });
        };
        const select = (event: XRInputSourceEvent): void => {
          const pose = event.frame.getPose(event.inputSource.targetRaySpace, reference);
          if (pose != null && this.presented_id !== null) this.notify({ kind: "xr_select", display_resource_id: id,
            presentation_id: this.presented_id, pointer: { source_id: this.sourceId(event.inputSource), tracking_from_ray: xrPose(pose.transform) }, timestamp_ms: performance.now() });
        };
        reference.addEventListener("reset", reset);
        handle.session.addEventListener("select", select);
        const detached = handle.detach;
        handle.detach = () => {
          detached();
          reference.removeEventListener("reset", reset);
          handle.session.removeEventListener("select", select);
        };
        this.display_id = id;
        this.display_handles.add(id);
        this.renderer.setAnimationLoop(this.frame);
      };
      void start().then(() => {
        this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "started", resource_id: id } });
      }, (cause: unknown) => {
        this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
      });
      return;
    }
    const id = request.request_id;
    try {
      this.display_id = id;
      this.display_handles.add(id);
      this.renderer.xr.enabled = false;
      this.renderer.setAnimationLoop(this.frame);
    } catch (cause) {
      this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: errorMessage(cause) } });
      return;
    }
    this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "started", resource_id: id } });
  }

  private async stopDisplay(id: number | null): Promise<void> {
    const failures: unknown[] = [];
    if (id === null || this.display_id === id) {
      this.display_id = null;
      try { this.renderer.setAnimationLoop(null); } catch (cause) { failures.push(cause); }
    }
    const handle = id === null ? undefined : this.sessions.get(id);
    if (handle !== undefined) {
      if (!handle.ended) {
        try { await handle.session.end(); } catch (cause) { failures.push(cause); }
      }
      if (this.renderer.xr.getSession() === handle.session) {
        try { await this.renderer.xr.setSession(null); } catch (cause) { failures.push(cause); }
      }
    }
    if (failures.length > 0) throw new AggregateError(failures, "Display stop failed");
  }

  private async releaseDisplay(id: number): Promise<void> {
    const failures: unknown[] = [];
    try { await this.stopDisplay(id); } catch (cause) { failures.push(cause); }
    const handle = this.sessions.get(id);
    this.sessions.delete(id);
    this.display_handles.delete(id);
    try { handle?.detach(); } catch (cause) { failures.push(cause); }
    if (failures.length > 0) throw new AggregateError(failures, "Display cleanup failed");
  }

  private sourceId(source: XRInputSource): number {
    let id = this.sources.get(source);
    if (id !== undefined) return id;
    if (this.next_source >= 0xffff_ffff) throw new Error("XR input source handles exhausted");
    id = ++this.next_source;
    this.sources.set(source, id);
    return id;
  }

  private readonly frame = (now: number, frame?: XRFrame): void => {
    if (this.disposed) return;
    try { this.observeFrame(now, frame); }
    catch (cause) { this.notify({ kind: "renderer_failed", renderer_resource_id: this.id, cause: errorMessage(cause) }); }
  };

  private observeFrame(now: number, frame: XRFrame | undefined): void {
    for (const observation of this.timer.poll()) {
      this.notify({ kind: "graphics", renderer_resource_id: this.id, observation: observation.kind === "sample"
        ? { kind: "gpu_completed", query: observation.source, gpu_work_ms: observation.milliseconds }
        : observation });
    }
    const gamepads = typeof navigator.getGamepads !== "function" ? [] : navigator.getGamepads().flatMap(gamepad => gamepad === null ? [] : [{
      identity: { slot_low: gamepad.index, slot_high: 0, generation: this.gamepad_generations.get(gamepad.index) ?? 0 },
      connected: gamepad.connected, mapping: gamepad.mapping,
      axes: gamepad.axes.map(axis => Number.isFinite(axis) ? axis : null), buttons: gamepad.buttons.map(button => button.pressed)
    }]);
    const handle = this.display_id === null ? undefined : this.sessions.get(this.display_id);
    const reference = handle?.reference;
    const viewer = reference == null || frame === undefined ? null : frame.getViewerPose(reference);
    const sources = reference == null || frame === undefined || handle === undefined ? [] : Array.from(handle.session.inputSources).map(source => {
      const pose = frame.getPose(source.targetRaySpace, reference);
      return { source_id: this.sourceId(source), target_ray_mode: source.targetRayMode, handedness: source.handedness,
        tracking_from_ray: pose == null ? null : xrPose(pose.transform),
        gamepad: source.gamepad == null ? null : { mapping: source.gamepad.mapping,
          axes: source.gamepad.axes.map(axis => Number.isFinite(axis) ? axis : null),
          buttons: source.gamepad.buttons.map(button => ({ pressed: button.pressed, touched: button.touched })) } };
    });
    this.notify({ kind: "frame", display_resource_id: this.display_id, timestamp_ms: now, gamepads,
      spatial: { gamepads, sources, presented_id: this.presented_id }, xr: handle === undefined ? null : {
        visibility: xrVisibility(handle.session), input_focused: handle.session.visibilityState === "visible",
        observation: { source: "runtime-derived", trackingFromHead: viewer == null ? null : xrPose(viewer.transform),
          views: viewer == null ? [] : viewer.views.map(view => ({ eye: view.eye === "left" || view.eye === "right" ? view.eye : null,
            trackingFromEye: xrPose(view.transform), projection: Array.from(view.projectionMatrix), depthRange: "negative-one-to-one" })) }
      } });
    this.draw(frame);
  }

  private draw(frame: XRFrame | undefined): void {
    const drawing = this.drawing;
    const active = this.active;
    if (drawing === null || active === null || this.canvas.hidden) return;
    const camera = drawing.scene.camera;
    if (camera === null || camera === undefined || camera.world_from_head == null) return;
    setPose(this.camera, camera.world_from_head);
    if (camera.screen_projection != null) this.camera.projectionMatrix.fromArray(camera.screen_projection);
    this.camera.projectionMatrixInverse.copy(this.camera.projectionMatrix).invert();
    const spatial = drawing.spatial;
    const menu = spatial?.menu;
    const head = spatial?.head_hud;
    if (this.menu_mesh !== null) {
      this.menu_mesh.visible = menu?.kind === "ready";
      if (menu?.kind === "ready") {
        this.paintMenu(menu);
        this.surfaceStyle(this.menu_mesh, menu.surface_style);
        setPose(this.menu_mesh, menu.world_from_panel);
        this.menu_mesh.scale.set(menu.paint.size.width, menu.paint.size.height, 1);
        active.scene.add(this.menu_mesh);
      }
    }
    if (this.head_mesh !== null) {
      this.head_mesh.visible = head?.kind === "ready";
      if (head?.kind === "ready") {
        this.paintHead(head);
        this.surfaceStyle(this.head_mesh, head.surface_style);
        setPose(this.head_mesh, head.world_from_surface);
        this.head_mesh.scale.set(head.paint.size.width, head.paint.size.height, 1);
        active.scene.add(this.head_mesh);
      }
    }
    const cursors = menu?.kind === "ready" ? menu.cursors : [];
    this.cursor_geometry ??= cursors.length === 0 ? null : new CircleGeometry(0.5, 32);
    while (this.cursor_meshes.length < cursors.length) {
      if (this.cursor_geometry === null) throw new Error("Cursor GPU primitive is unavailable");
      this.cursor_meshes.push(new Mesh(this.cursor_geometry, new MeshBasicMaterial({ side: DoubleSide, transparent: true, depthWrite: false })));
    }
    for (const [index, mesh] of this.cursor_meshes.entries()) {
      const cursor = cursors[index];
      mesh.visible = cursor !== undefined;
      if (cursor !== undefined) {
        setPose(mesh, cursor.world_from_cursor);
        mesh.scale.setScalar(cursor.diameter_m);
        mesh.material.color.setHex(cursor.color_srgb);
        mesh.material.opacity = cursor.opacity;
        mesh.material.depthTest = cursor.depth_test;
        mesh.renderOrder = cursor.render_order;
        active.scene.add(mesh);
      }
    }
    const query = drawing.graphics.query;
    if (query != null) this.timer.begin(query);
    let complete = false;
    const viewport = this.renderer.getViewport(new Vector4());
    const scissor = this.renderer.getScissor(new Vector4());
    const scissorTest = this.renderer.getScissorTest();
    const uncullable: Object3D[] = [];
    const failures: unknown[] = [];
    try {
      this.drawReflection(active, drawing);
      const eyes = camera.eyes;
      if (frame !== undefined && this.renderer.xr.isPresenting) {
        const array = this.renderer.xr.getCamera();
        const reference = this.renderer.xr.getReferenceSpace();
        const viewer = reference === null ? null : frame.getViewerPose(reference);
        viewer?.views.forEach((view, index) => {
          const target = array.cameras[index];
          const eye = eyes.find(eye => eye.eye === view.eye);
          if (target !== undefined && eye !== undefined) {
            setPose(target, eye.world_from_eye);
            target.projectionMatrix.fromArray(eye.projection);
            target.projectionMatrixInverse.copy(target.projectionMatrix).invert();
          }
        });
        setPose(array, camera.world_from_head);
        if (camera.culling_projection != null) {
          array.projectionMatrix.fromArray(camera.culling_projection);
          array.projectionMatrixInverse.copy(array.projectionMatrix).invert();
        } else active.scene.traverse(object => {
          if (object.frustumCulled) { uncullable.push(object); object.frustumCulled = false; }
        });
        this.renderer.render(active.scene, this.camera);
      } else if (eyes.length > 0) {
        this.renderer.setScissorTest(true);
        eyes.forEach(eye => {
          const viewport = eye.viewport_px;
          if (viewport == null) throw new Error("Configured stereo eye has no shared framebuffer viewport");
          setPose(this.camera, eye.world_from_eye);
          this.camera.projectionMatrix.fromArray(eye.projection);
          this.camera.projectionMatrixInverse.copy(this.camera.projectionMatrix).invert();
          this.renderer.setViewport(...viewport);
          this.renderer.setScissor(...viewport);
          this.renderer.render(active.scene, this.camera);
        });
      } else this.renderer.render(active.scene, this.camera);
      complete = true;
    } catch (cause) { failures.push(cause); }
    finally {
      for (const restore of [() => { this.renderer.setViewport(viewport); },
        () => { this.renderer.setScissor(scissor); }, () => { this.renderer.setScissorTest(scissorTest); }]) {
        try { restore(); } catch (cause) { failures.push(cause); }
      }
      for (const object of uncullable) object.frustumCulled = true;
      try { this.timer.end(complete && failures.length === 0); } catch (cause) { failures.push(cause); }
    }
    if (failures.length > 0) throw new AggregateError(failures, "Frame drawing failed");
    if (spatial != null) this.presented_id = spatial.presentation_id;
  }

  private drawReflection(active: PreparedResources, drawing: ApplicationDrawingOutput): void {
    const water = active.water_materials[0];
    if (water === undefined) throw new Error("Prepared water material is unavailable");
    uniform(water, "uReflectionEnabled", 0);
    const reflection = drawing.scene.reflection;
    if (reflection == null) return;
    setPose(this.reflection_camera, reflection.world_from_camera);
    this.reflection_camera.projectionMatrix.fromArray(reflection.projection);
    this.reflection_camera.projectionMatrixInverse.copy(this.reflection_camera.projectionMatrix).invert();
    uniform(water, "uReflectionMatrix", new Matrix4().fromArray(reflection.projection_from_world));
    const target = this.renderer.getRenderTarget();
    const background = active.scene.background;
    const alpha = this.renderer.getClearAlpha();
    const color = this.renderer.getClearColor(new Color());
    const xr = this.renderer.xr.enabled;
    const shadowAutoUpdate = this.renderer.shadowMap.autoUpdate;
    const shadowNeedsUpdate = this.renderer.shadowMap.needsUpdate;
    const viewport = this.renderer.getViewport(new Vector4());
    const scissor = this.renderer.getScissor(new Vector4());
    const scissorTest = this.renderer.getScissorTest();
    const failures: unknown[] = [];
    try {
      active.scene.background = null;
      this.renderer.xr.enabled = false;
      if (active.sun.shadow.map !== null) {
        this.renderer.shadowMap.autoUpdate = false;
        this.renderer.shadowMap.needsUpdate = false;
      }
      this.renderer.setRenderTarget(active.reflection);
      this.renderer.setScissorTest(false);
      this.renderer.setClearColor(0x000000, 0);
      this.renderer.clear();
      this.renderer.render(active.scene, this.reflection_camera);
    } catch (cause) { failures.push(cause); }
    finally {
      for (const restore of [() => { active.scene.background = background; }, () => { this.renderer.xr.enabled = xr; },
        () => { this.renderer.shadowMap.autoUpdate = shadowAutoUpdate; this.renderer.shadowMap.needsUpdate = shadowNeedsUpdate; },
        () => { this.renderer.setRenderTarget(target); }, () => { this.renderer.setClearColor(color, alpha); },
        () => { this.renderer.setViewport(viewport); }, () => { this.renderer.setScissor(scissor); },
        () => { this.renderer.setScissorTest(scissorTest); }]) {
        try { restore(); } catch (cause) { failures.push(cause); }
      }
    }
    if (failures.length > 0) throw new AggregateError(failures, "Reflection drawing failed");
    uniform(water, "uReflectionEnabled", 1);
  }

  private surfaceStyle(mesh: Mesh<PlaneGeometry, MeshBasicMaterial>, style: XrSurfaceStyle): void {
    const material = mesh.material;
    if (material.transparent !== style.transparent) {
      material.transparent = style.transparent;
      material.needsUpdate = true;
    }
    material.depthTest = style.depth_test;
    material.depthWrite = style.depth_write;
    mesh.renderOrder = style.render_order;
  }

  private paintMenu(menu: Extract<NonNullable<ApplicationDrawingOutput["spatial"]>["menu"], { kind: "ready" }>): void {
    if (this.menu_mesh === null) return;
    const context = this.paintContext(this.menu_canvas, menu.canvas_size, this.menu_mesh);
    const paint = menu.paint;
    const scaleX = this.menu_canvas.width / paint.size.width;
    const scaleY = this.menu_canvas.height / paint.size.height;
    context.setFillStyle(paint.background);
    context.setGlobalAlpha(paint.opacity);
    context.fillRect(0, 0, this.menu_canvas.width, this.menu_canvas.height);
    context.setGlobalAlpha(1);
    context.save();
    try {
      const clip = paint.content.clip;
      context.beginPath(); context.rect(clip.x * scaleX, clip.y * scaleY, clip.width * scaleX, clip.height * scaleY); context.clip();
      paintSurface(context, paint.content.items, scaleX, scaleY);
    } finally { context.restore(); }
    paintSurface(context, paint.footer, scaleX, scaleY);
    if (this.menu_mesh.material.map !== null) this.menu_mesh.material.map.needsUpdate = true;
  }

  private paintHead(head: Extract<NonNullable<ApplicationDrawingOutput["spatial"]>["head_hud"], { kind: "ready" }>): void {
    if (this.head_mesh === null) return;
    const context = this.paintContext(this.head_canvas, head.paint.canvasSize, this.head_mesh);
    paintSurface(context, head.paint.items, this.head_canvas.width / head.paint.size.width, this.head_canvas.height / head.paint.size.height);
    if (this.head_mesh.material.map !== null) this.head_mesh.material.map.needsUpdate = true;
  }

  private paintContext(canvas: HTMLCanvasElement, size: readonly [number, number], mesh: Mesh<PlaneGeometry, MeshBasicMaterial>) {
    if (canvas.width !== size[0] || canvas.height !== size[1]) {
      canvas.width = size[0]; canvas.height = size[1];
      mesh.material.map?.dispose();
      const texture = new CanvasTexture(canvas);
      texture.colorSpace = SRGBColorSpace; texture.minFilter = LinearFilter; texture.generateMipmaps = false;
      mesh.material.map = texture;
    }
    const context = canvas.getContext("2d");
    if (context === null) throw new Error("Canvas painting is unavailable");
    const surface = browserSurfaceContext(context);
    surface.clearRect(0, 0, canvas.width, canvas.height);
    return surface;
  }

  private readonly contextLost = (): void => {
    this.renderer.setAnimationLoop(null);
    this.notify({ kind: "graphics", renderer_resource_id: this.id, observation: { kind: "context_lost" } });
  };
  private readonly pointerMove = (event: PointerEvent): void => {
    this.notify({ kind: "pointer_motion", display_resource_id: this.display_id, delta_px: [event.movementX, event.movementY], buttons: event.buttons });
  };
  private readonly contextMenu = (event: Event): void => { event.preventDefault(); };
  private readonly gamepadConnection = (event: GamepadEvent): void => {
    const generation = (this.gamepad_generations.get(event.gamepad.index) ?? 0) + 1;
    if (generation > 0xffff_ffff) throw new Error("Gamepad connection handles exhausted");
    this.gamepad_generations.set(event.gamepad.index, generation);
  };

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const failures: unknown[] = [];
    const release = (operation: () => void): void => { try { operation(); } catch (cause) { failures.push(cause); } };
    release(() => { this.renderer.setAnimationLoop(null); });
    release(() => { this.canvas.removeEventListener("webglcontextlost", this.contextLost); });
    release(() => { this.canvas.removeEventListener("pointermove", this.pointerMove); });
    release(() => { this.canvas.removeEventListener("contextmenu", this.contextMenu); });
    release(() => { window.removeEventListener("gamepadconnected", this.gamepadConnection); });
    release(() => { window.removeEventListener("gamepaddisconnected", this.gamepadConnection); });
    release(() => { this.renderer.xr.removeEventListener("sessionstart", this.platformChanged); });
    release(() => { this.renderer.xr.removeEventListener("sessionend", this.platformChanged); });
    release(() => { this.timer.clear(); });
    for (const candidate of this.resources.values()) release(() => { this.releaseCandidate(candidate); });
    this.resources.clear();
    for (const material of this.materials.values()) release(() => { material.dispose(); });
    for (const ranges of this.range_geometry.values()) for (const geometry of ranges) release(() => { geometry.dispose(); });
    for (const mesh of [this.menu_mesh, this.head_mesh]) if (mesh !== null) {
      release(() => { mesh.geometry.dispose(); });
      release(() => { mesh.material.map?.dispose(); });
      release(() => { mesh.material.dispose(); });
    }
    for (const mesh of this.cursor_meshes) release(() => { mesh.material.dispose(); });
    release(() => { this.cursor_geometry?.dispose(); });
    release(() => { this.buffers.dispose(); });
    release(() => { this.renderer.dispose(); });
    release(() => { this.canvas.remove(); });
    if (failures.length > 0) throw new AggregateError(failures, "Renderer cleanup failed");
  }
}

function setPose(object: Object3D, pose: Pose): void {
  object.position.set(pose.position.x, pose.position.y, pose.position.z);
  object.quaternion.set(pose.orientation.x, pose.orientation.y, pose.orientation.z, pose.orientation.w);
  object.updateMatrixWorld(true);
}

function errorMessage(cause: unknown): string {
  if (cause instanceof AggregateError) return `${cause.name}: ${cause.message} (${cause.errors.map(errorMessage).join("; ")})`;
  return cause instanceof Error ? `${cause.name}: ${cause.message}${cause.cause === undefined ? "" : `: ${errorMessage(cause.cause)}`}` : String(cause);
}

function uniform(material: ShaderMaterial, name: string, value: unknown): void {
  const slot = material.uniforms[name];
  if (slot === undefined) throw new Error(`Missing GPU uniform ${name}`);
  slot.value = value;
}

function xrPose(transform: XRRigidTransform): Pose {
  const { position, orientation } = transform;
  return { position: { x: position.x, y: position.y, z: position.z },
    orientation: { w: orientation.w, x: orientation.x, y: orientation.y, z: orientation.z } };
}

function xrVisibility(session: XRSession): "visible" | "blurred" | "hidden" {
  return session.visibilityState === "visible-blurred" ? "blurred" : session.visibilityState;
}
