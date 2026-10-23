use super::{READY_CONFIRMATION_FRAMES, Verification};
use crate::app::{screen_render::timing::shader_is_pending, water::NearWaterSurface};
use bevy::{
    asset::{DependencyLoadState, LoadState, RecursiveDependencyLoadState},
    material::descriptor::PipelineDescriptor,
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, RenderApp,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache},
    },
};

pub(super) struct VerificationRenderPlugin;

impl Plugin for VerificationRenderPlugin {
    fn build(&self, app: &mut App) {
        crate::app::water::verification::install(app);
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, observe_render_readiness);
        } else if let Some(mut verification) = app.world_mut().get_resource_mut::<Verification>() {
            verification.readiness.error = Some("Verification RenderApp is unavailable".into());
        }
    }
}

#[derive(Default, Debug)]
pub(super) struct RenderReadiness {
    pub(super) observed_frames: usize,
    pub(super) pending_pipelines: usize,
    pub(super) water_pipelines: usize,
    pub(super) terrain_pipelines: usize,
    pub(super) assets_loaded: bool,
    pub(super) confirmation_frames: usize,
    pub(super) error: Option<String>,
}

impl RenderReadiness {
    pub(super) fn confirm_frame(&mut self) {
        if self.assets_loaded
            && self.water_pipelines > 0
            && self.terrain_pipelines > 0
            && self.pending_pipelines == 0
            && self.error.is_none()
        {
            self.confirmation_frames =
                (self.confirmation_frames + 1).min(READY_CONFIRMATION_FRAMES);
        } else {
            self.confirmation_frames = 0;
        }
    }

    pub(super) fn can_capture(&self) -> bool {
        self.confirmation_frames == READY_CONFIRMATION_FRAMES && self.error.is_none()
    }
}

fn observe_render_readiness(
    mut main_world: ResMut<MainWorld>,
    pipelines: Option<Res<PipelineCache>>,
    meshes: Option<Res<RenderAssets<RenderMesh>>>,
) {
    let Some(verification) = main_world.get_resource::<Verification>() else {
        return;
    };
    let mut readiness = RenderReadiness {
        observed_frames: verification.readiness.observed_frames + 1,
        confirmation_frames: verification.readiness.confirmation_frames,
        error: verification.readiness.error.clone(),
        assets_loaded: true,
        ..default()
    };
    let scene_shaders = verification.scene_shaders.clone();
    let asset_server = main_world.resource::<AssetServer>();
    for shader in &scene_shaders {
        readiness.assets_loaded &= asset_server.is_loaded_with_dependencies(shader.id());
        if let Some((load, dependencies, recursive)) = asset_server.get_load_states(shader.id()) {
            let error = match (load, dependencies, recursive) {
                (LoadState::Failed(error), _, _)
                | (_, DependencyLoadState::Failed(error), _)
                | (_, _, RecursiveDependencyLoadState::Failed(error)) => Some(error),
                _ => None,
            };
            if let Some(error) = error {
                readiness.error.get_or_insert_with(|| {
                    format!("Scene shader asset {:?}: {error}", shader.path())
                });
            }
        }
    }
    let mut near = main_world.query_filtered::<&Mesh3d, With<NearWaterSurface>>();
    let near_meshes: Vec<_> = near.iter(&main_world).map(|mesh| mesh.0.id()).collect();
    if near_meshes.is_empty() {
        readiness.assets_loaded = false;
    }
    for mesh in near_meshes {
        readiness.assets_loaded &= meshes
            .as_ref()
            .is_some_and(|meshes| meshes.get(mesh).is_some());
    }
    if let Some(pipelines) = pipelines {
        readiness.pending_pipelines = pipelines.waiting_pipelines().count();
        let mut unresolved_pipelines = 0;
        for pipeline in pipelines.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => {
                    if let PipelineDescriptor::RenderPipelineDescriptor(descriptor) =
                        &pipeline.descriptor
                        && let Some(fragment) = &descriptor.fragment
                    {
                        if fragment.shader.id() == scene_shaders[0].id() {
                            readiness.water_pipelines += 1;
                        }
                        if fragment.shader.id() == scene_shaders[1].id() {
                            readiness.terrain_pipelines += 1;
                        }
                    }
                }
                CachedPipelineState::Err(error) if shader_is_pending(error) => {
                    unresolved_pipelines += 1;
                }
                CachedPipelineState::Err(error) => {
                    readiness.error.get_or_insert_with(|| {
                        format!("GPU pipeline {:?}: {error:?}", pipeline.descriptor)
                    });
                }
                CachedPipelineState::Queued | CachedPipelineState::Creating(_) => {
                    unresolved_pipelines += 1;
                }
            }
        }
        readiness.pending_pipelines = readiness.pending_pipelines.max(unresolved_pipelines);
    } else {
        readiness.assets_loaded = false;
    }
    readiness.confirm_frame();
    if let Some(mut verification) = main_world.get_resource_mut::<Verification>() {
        verification.readiness = readiness;
    }
}
