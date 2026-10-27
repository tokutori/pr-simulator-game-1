use super::*;
use bevy::render::renderer::{RenderGraph, RenderGraphSystems, RenderQueue};
use bevy::render::{
    ExtractSchedule, MainWorld, RenderApp, render_asset::RenderAssets, texture::GpuImage,
};
use bevy_mod_openxr::layer_builder::{
    CompositionLayerProjection, CompositionLayerProjectionView, SwapchainSubImage,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(super) struct Blit {
    pub frame: u64,
    pub source: AssetId<Image>,
    pub presentation: Option<u32>,
}

#[derive(Default)]
pub(super) struct Completion {
    pub retired: u64,
    pub done: Option<(u64, Option<u32>, bool)>,
    pub failure: Option<String>,
    pub ended: Option<String>,
    pub lost: bool,
    gpu_done: bool,
}

struct Pending {
    frame: worker::Frame,
    source: Option<Blit>,
    begun: bool,
    acquired: Option<u32>,
    waited: bool,
    submitted: bool,
}

#[derive(Resource, Default)]
struct Packet {
    owned: Option<Arc<Mutex<OxrRenderResources>>>,
    pending: Option<Pending>,
    last: Option<u64>,
    closing: bool,
    drop_resources: bool,
    revision: u64,
    completion: Arc<Mutex<Completion>>,
}

pub(super) fn install(app: &mut App) {
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<Packet>()
            .add_systems(ExtractSchedule, extract)
            .add_systems(
                RenderGraph,
                (
                    advance.before(RenderGraphSystems::Begin),
                    copy.after(RenderGraphSystems::Submit),
                ),
            );
    }
}

fn extract(mut main: ResMut<MainWorld>, mut packet: ResMut<Packet>) {
    let Some(mut io) = main.get_resource_mut::<XrIo>() else {
        return;
    };
    if !Arc::ptr_eq(&packet.completion, &io.completion) {
        packet.last = None;
        packet.completion = io.completion.clone();
    }
    if io
        .session
        .as_ref()
        .is_some_and(|session| session.permission_granted)
        && let Some(owned) = io.render.take()
    {
        packet.owned = Some(owned);
    }
    packet.closing = io.closing.is_some();
    packet.drop_resources = io
        .session
        .as_ref()
        .is_some_and(|session| session.stopped && session.retiring());
    packet.revision = io.revision;
    if packet.pending.is_none()
        && let Some(frame) = &io.frame
        && packet.last != Some(frame.id)
    {
        packet.last = Some(frame.id);
        packet.pending = Some(Pending {
            frame: frame.clone(),
            source: None,
            begun: false,
            acquired: None,
            waited: false,
            submitted: false,
        });
    }
    if let Some(pending) = &mut packet.pending {
        pending.source = io
            .blit
            .as_ref()
            .filter(|blit| blit.frame == pending.frame.id)
            .cloned();
    }
}

fn fault(packet: &Packet, cause: impl std::fmt::Display) {
    let mut completion = packet.completion.lock().unwrap();
    if completion.failure.is_none() {
        completion.failure = Some(format!("OpenXR render I/O: {cause}"));
    }
}

fn finish(packet: &mut Packet, submitted: bool) -> Result<(), String> {
    let completion = packet.completion.clone();
    let pending = packet
        .pending
        .as_ref()
        .ok_or("OpenXR has no current frame")?;
    let handle = packet
        .owned
        .as_ref()
        .ok_or("OpenXR render ownership is missing")?;
    let mut owned = handle.lock().map_err(|cause| cause.to_string())?;
    if pending.acquired.is_some() {
        let status = owned
            .swapchain
            .release_image()
            .map_err(|cause| api_error(&completion, cause))?;
        record_status(&completion, status);
    }
    let OxrRenderResources {
        swapchain,
        frame_stream,
        graphics_info,
        ..
    } = &mut *owned;
    let projected: Vec<_> = pending
        .frame
        .views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            CompositionLayerProjectionView::new()
                .pose(view.pose)
                .fov(view.fov)
                .sub_image(
                    SwapchainSubImage::new()
                        .swapchain(swapchain)
                        .image_array_index(index as u32)
                        .image_rect(openxr::Rect2Di {
                            offset: openxr::Offset2Di { x: 0, y: 0 },
                            extent: openxr::Extent2Di {
                                width: graphics_info.resolution.x as i32,
                                height: graphics_info.resolution.y as i32,
                            },
                        }),
                )
        })
        .collect();
    let layer = CompositionLayerProjection::new()
        .reference_space(&pending.frame.base)
        .views(&projected);
    let layers = if submitted {
        vec![&layer as &dyn bevy_mod_openxr::layer_builder::CompositionLayer]
    } else {
        Vec::new()
    };
    let status = frame_stream
        .end(
            pending.frame.time,
            openxr::EnvironmentBlendMode::OPAQUE,
            &layers,
        )
        .map_err(|cause| api_error(&completion, cause))?;
    record_status(&completion, status);
    drop(layer);
    drop(projected);
    drop(owned);
    let pending = packet.pending.take().unwrap();
    packet.completion.lock().unwrap().done = Some((
        pending.frame.id,
        pending.source.and_then(|source| source.presentation),
        submitted,
    ));
    Ok(())
}

fn record_status(completion: &Arc<Mutex<Completion>>, status: openxr::sys::Result) {
    if status == openxr::sys::Result::SESSION_LOSS_PENDING {
        let mut completion = completion.lock().unwrap();
        completion.lost = true;
        completion
            .ended
            .get_or_insert_with(|| format!("OpenXR: {status}"));
    }
}

fn api_error(
    completion: &Arc<Mutex<Completion>>,
    cause: bevy_mod_openxr::error::OxrError,
) -> String {
    if matches!(
        cause,
        bevy_mod_openxr::error::OxrError::OpenXrError(openxr::sys::Result::ERROR_SESSION_LOST)
    ) {
        let mut completion = completion.lock().unwrap();
        completion.lost = true;
        completion.ended.get_or_insert_with(|| cause.to_string());
    }
    cause.to_string()
}

fn advance(mut packet: ResMut<Packet>) {
    if packet.completion.lock().unwrap().failure.is_some() {
        return;
    }
    let result = (|| {
        let completion = packet.completion.clone();
        let lost = completion.lock().unwrap().lost;
        if lost && let Some(pending) = &packet.pending {
            if pending.submitted && !completion.lock().unwrap().gpu_done {
                return Ok(());
            }
            let pending = packet.pending.take().unwrap();
            let mut completion = completion.lock().unwrap();
            completion.gpu_done = false;
            completion.done = Some((pending.frame.id, None, false));
        }
        if packet.pending.is_some() && packet.owned.is_some() {
            if packet.pending.as_ref().unwrap().submitted {
                let done = packet.completion.lock().unwrap().gpu_done;
                if !done {
                    return Ok(());
                }
                finish(&mut packet, true)?;
                packet.completion.lock().unwrap().gpu_done = false;
            } else {
                let mut closing = packet.closing;
                let Packet { pending, owned, .. } = &mut *packet;
                let pending = pending.as_mut().unwrap();
                let mut owned = owned
                    .as_ref()
                    .unwrap()
                    .lock()
                    .map_err(|cause| cause.to_string())?;
                if !pending.begun {
                    let status = owned
                        .frame_stream
                        .begin()
                        .map_err(|cause| api_error(&completion, cause.into()))?;
                    record_status(&completion, status);
                    closing |= status == openxr::sys::Result::SESSION_LOSS_PENDING;
                    pending.begun = true;
                }
                if pending.acquired.is_none()
                    && (!pending.frame.should_render || closing || pending.source.is_none())
                {
                    drop(owned);
                    finish(&mut packet, false)?;
                } else {
                    if pending.acquired.is_none() {
                        let (index, status) = owned
                            .swapchain
                            .acquire_image()
                            .map_err(|cause| api_error(&completion, cause))?;
                        record_status(&completion, status);
                        closing |= status == openxr::sys::Result::SESSION_LOSS_PENDING;
                        pending.acquired = Some(index);
                    }
                    if !pending.waited {
                        let status = owned
                            .swapchain
                            .wait_image(openxr::Duration::from_nanos(50_000_000))
                            .map_err(|cause| api_error(&completion, cause))?;
                        if status == openxr::sys::Result::TIMEOUT_EXPIRED {
                            return Ok(());
                        }
                        pending.waited = true;
                        if status == openxr::sys::Result::SESSION_LOSS_PENDING {
                            record_status(&completion, status);
                            drop(owned);
                            finish(&mut packet, false)?;
                            return Ok(());
                        }
                    }
                    drop(owned);
                    if closing {
                        finish(&mut packet, false)?;
                    }
                }
            }
        }
        if packet.drop_resources && packet.pending.is_none() {
            // Worker retains the final owning Arc until the Release command.
            packet.owned.take();
            packet.completion.lock().unwrap().retired = packet.revision;
        }
        Ok::<_, String>(())
    })();
    if let Err(cause) = result
        && !packet.completion.lock().unwrap().lost
    {
        fault(&packet, cause);
    }
}

fn copy(world: &mut World) {
    let mut packet = world.remove_resource::<Packet>().unwrap();
    let result = (|| {
        if packet.closing || {
            let completion = packet.completion.lock().unwrap();
            completion.failure.is_some() || completion.lost
        } {
            return Ok(());
        }
        let Some(pending) = &packet.pending else {
            return Ok(());
        };
        if !pending.waited || pending.submitted {
            return Ok(());
        }
        let Some(blit) = &pending.source else {
            return finish(&mut packet, false);
        };
        let handle = packet
            .owned
            .as_ref()
            .ok_or("OpenXR render ownership is missing")?;
        let owned = handle.lock().map_err(|cause| cause.to_string())?;
        let extent = owned.graphics_info.resolution;
        let Some(source) = world.resource::<RenderAssets<GpuImage>>().get(blit.source) else {
            drop(owned);
            return finish(&mut packet, false);
        };
        if source.texture_descriptor.size.width != extent.x * 2
            || source.texture_descriptor.size.height != extent.y
        {
            drop(owned);
            return finish(&mut packet, false);
        }
        if source.texture_descriptor.format != wgpu::TextureFormat::Rgba8UnormSrgb {
            return Err("OpenXR source framebuffer format changed".into());
        }
        let index = pending
            .acquired
            .ok_or("OpenXR image has not been acquired")?;
        let target = owned
            .images
            .get(index as usize)
            .ok_or("OpenXR acquired an unknown image")?;
        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("OpenXR stereo image copy"),
        });
        for eye in 0..2 {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &source.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: eye * extent.x,
                        y: 0,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: target,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: eye },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: extent.x,
                    height: extent.y,
                    depth_or_array_layers: 1,
                },
            );
            let view = target.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: eye,
                array_layer_count: Some(1),
                ..default()
            });
            // OpenXR requires COLOR_ATTACHMENT_OPTIMAL before Vulkan image release.
            drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OpenXR final image layout"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..default()
            }));
        }
        queue.submit([encoder.finish()]);
        let completion = packet.completion.clone();
        queue.on_submitted_work_done(move || {
            completion.lock().unwrap().gpu_done = true;
        });
        packet.pending.as_mut().unwrap().submitted = true;
        Ok::<_, String>(())
    })();
    if let Err(cause) = result {
        fault(&packet, cause);
    }
    world.insert_resource(packet);
}
