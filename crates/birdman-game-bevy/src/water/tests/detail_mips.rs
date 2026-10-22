use super::*;

#[test]
fn detail_mip_admission_tracks_gpu_texture_generation_and_discards_retired_images() {
    let mut images = Assets::<Image>::default();
    let near = images.add(Image::default());
    let far = images.add(Image::default());
    let sky = images.add(Image::default());
    let first_texture = TextureId::new();
    let replacement_texture = TextureId::new();
    let mut state = LakeDetailMips {
        active: vec![near.id(), far.id(), sky.id()],
        recorded: Vec::new(),
        failure: None,
    };
    assert!(state.pending(near.id(), first_texture));
    state.recorded.push((near.id(), first_texture));
    state.recorded.push((sky.id(), TextureId::new()));
    assert!(!state.pending(near.id(), first_texture));
    assert!(state.pending(near.id(), replacement_texture));
    state.active = vec![far.id()];
    state.retain_active();
    assert!(state.recorded.is_empty());
}

#[test]
fn detail_mips_match_only_the_official_d2_shader_and_pass_entry_points() {
    let mut shaders = Assets::<Shader>::default();
    let official = shaders.add(Shader::from_wgsl("", "official"));
    let other = shaders.add(Shader::from_wgsl("", "other"));
    let mut descriptor = ComputePipelineDescriptor {
        shader: official.clone(),
        entry_point: Some("downsample_first".into()),
        ..default()
    };
    assert!(mip_pipeline_matches(
        &descriptor,
        &official,
        "downsample_first"
    ));
    assert!(!mip_pipeline_matches(
        &descriptor,
        &official,
        "downsample_second"
    ));
    assert!(!mip_pipeline_matches(
        &descriptor,
        &other,
        "downsample_first"
    ));
    for (entry, separate_pass) in [
        ("downsample_first", "FIRST_PASS"),
        ("downsample_second", "SECOND_PASS"),
    ] {
        for combined in [false, true] {
            descriptor.entry_point = Some(entry.into());
            descriptor.shader_defs = vec![
                ShaderDefVal::Int("SUBGROUP_SUPPORT".into(), 1),
                ShaderDefVal::Int(
                    if combined {
                        "COMBINE_BIND_GROUP"
                    } else {
                        separate_pass
                    }
                    .into(),
                    1,
                ),
            ];
            assert!(mip_pipeline_matches(&descriptor, &official, entry));
        }
    }
    descriptor.entry_point = None;
    assert!(!mip_pipeline_matches(
        &descriptor,
        &official,
        "downsample_first"
    ));
}

#[test]
fn detail_mip_pipeline_waits_for_imports_but_stops_queueing_confirmed_failure() {
    let mut shaders = Assets::<Shader>::default();
    let official = shaders.add(Shader::from_wgsl("", "official"));
    let other = shaders.add(Shader::from_wgsl("", "other"));
    let mut images = Assets::<Image>::default();
    let near = images.add(Image::default());
    let replacement = images.add(Image::default());
    let descriptor = ComputePipelineDescriptor {
        shader: official.clone(),
        entry_point: Some("downsample_first".into()),
        ..default()
    };
    let active = [near.id()];
    for pending in [
        CachedPipelineState::Queued,
        CachedPipelineState::Creating(
            bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new)
                .spawn(async { Err(ShaderCacheError::ShaderImportNotYetAvailable) }),
        ),
        CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(official.id())),
        CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable),
    ] {
        assert_eq!(
            mip_pipeline_ready(
                &descriptor,
                &pending,
                &official,
                "downsample_first",
                &active
            ),
            Ok(false)
        );
    }
    let failed = CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
        "original GPU validation cause".into(),
    ));
    assert_eq!(
        mip_pipeline_ready(&descriptor, &failed, &other, "downsample_first", &active),
        Ok(false)
    );
    let cause = mip_pipeline_ready(&descriptor, &failed, &official, "downsample_first", &active)
        .unwrap_err();
    assert_eq!(cause.shader, official.id());
    assert_eq!(cause.entry, "downsample_first");
    assert_eq!(cause.images, active);
    assert!(cause.cause.contains("CreateShaderModule"));
    assert!(cause.cause.contains("original GPU validation cause"));
    let mut state = LakeDetailMips {
        active: active.to_vec(),
        recorded: Vec::new(),
        failure: Some(cause),
    };
    assert!(!state.pending(near.id(), TextureId::new()));
    state.retain_active();
    assert!(state.failure.is_some());
    state.active = vec![replacement.id()];
    state.retain_active();
    assert!(state.failure.is_none());
    assert!(state.pending(replacement.id(), TextureId::new()));
}

#[test]
fn detail_mip_readiness_ignores_foreign_array_variants_while_d2_is_queued() {
    let mut shaders = Assets::<Shader>::default();
    let official = shaders.add(Shader::from_wgsl("", "official"));
    let active = [Handle::<Image>::default().id()];
    for entry in ["downsample_first", "downsample_second"] {
        let descriptor = ComputePipelineDescriptor {
            shader: official.clone(),
            shader_defs: vec![
                ShaderDefVal::Int("SUBGROUP_SUPPORT".into(), 1),
                ShaderDefVal::Int("COMBINE_BIND_GROUP".into(), 1),
            ],
            entry_point: Some(entry.into()),
            ..default()
        };
        let failed = CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
            "target d2 validation cause".into(),
        ));
        let cause =
            mip_pipeline_ready(&descriptor, &failed, &official, entry, &active).unwrap_err();
        assert_eq!(cause.entry, entry);
        assert!(cause.cause.contains("target d2 validation cause"));
        for array_definition in [
            ShaderDefVal::Bool("ARRAY_TEXTURE".into(), true),
            ShaderDefVal::Bool("ARRAY_TEXTURE".into(), false),
            ShaderDefVal::Int("ARRAY_TEXTURE".into(), 1),
            ShaderDefVal::UInt("ARRAY_TEXTURE".into(), 1),
        ] {
            let mut foreign = descriptor.clone();
            foreign.shader_defs.push(array_definition);
            assert!(!mip_pipeline_matches(&foreign, &official, entry));
            let mut ready = mip_pipeline_ready(
                &descriptor,
                &CachedPipelineState::Queued,
                &official,
                entry,
                &active,
            )
            .unwrap();
            for state in [
                CachedPipelineState::Queued,
                CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(official.id())),
                CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable),
                CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
                    "foreign array validation cause".into(),
                )),
            ] {
                ready |= mip_pipeline_ready(&foreign, &state, &official, entry, &active).unwrap();
            }
            assert!(!ready);
        }
    }
}
