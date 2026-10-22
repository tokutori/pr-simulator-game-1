use super::*;

#[test]
fn static_water_meshes_preserve_coverage_budget_and_an_open_far_center() {
    let near = near_mesh();
    assert_eq!(near.count_vertices(), 50_625);
    let Some(Indices::U32(near_indices)) = near.indices() else {
        panic!("Expected uint32 near mesh indices");
    };
    assert_eq!(near_indices.len(), 100_352 * 3);
    for [position_x, position_y, position_z] in mesh_positions(&near) {
        assert!(position_x.abs() <= 3000.0 && position_z.abs() <= 3000.0);
        assert_eq!(*position_y, 0.0);
    }
    let far = far_mesh();
    assert_eq!(far.count_vertices(), 16);
    let Some(Indices::U32(far_indices)) = far.indices() else {
        panic!("Expected uint32 far mesh indices");
    };
    assert_eq!(far_indices.len(), 24);
    let positions = mesh_positions(&far);
    for triangle in far_indices.chunks_exact(3) {
        let first = Vec3::from_array(positions[triangle[0] as usize]);
        let second = Vec3::from_array(positions[triangle[1] as usize]);
        let third = Vec3::from_array(positions[triangle[2] as usize]);
        assert!((second - first).cross(third - first).y > 0.0);
    }
    for rectangle in positions.chunks_exact(4) {
        let minimum_x = rectangle
            .iter()
            .map(|position| position[0])
            .fold(f32::INFINITY, f32::min);
        let maximum_x = rectangle
            .iter()
            .map(|position| position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let minimum_z = rectangle
            .iter()
            .map(|position| position[2])
            .fold(f32::INFINITY, f32::min);
        let maximum_z = rectangle
            .iter()
            .map(|position| position[2])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            maximum_x <= -3000.0
                || minimum_x >= 3000.0
                || maximum_z <= -3000.0
                || minimum_z >= 3000.0
        );
    }
    assert!(positions.iter().any(|position| position[0] == -90_000.0));
    assert!(positions.iter().any(|position| position[2] == 90_000.0));
}

#[test]
fn geometry_phases_are_fixed_and_detail_seed_affects_only_shared_images() {
    let original = WaterMaterial::registered(false).unwrap();
    let mut condition = EnvironmentTarget::exhibition()
        .resolve()
        .unwrap()
        .condition();
    condition.waves.pattern_seed += 1;
    let changed = WaterMaterial::from_environment(condition, false).unwrap();
    assert_eq!(original.geometry_waves, changed.geometry_waves);
    assert_eq!(original.geometry_motion, changed.geometry_motion);
    assert_ne!(
        original.visual_condition.pattern_seed(),
        changed.visual_condition.pattern_seed()
    );
    condition.waves.fetch_m *= 2.0;
    assert_ne!(
        original.geometry_waves,
        WaterMaterial::from_environment(condition, false)
            .unwrap()
            .geometry_waves
    );
}

#[test]
fn invalid_shared_condition_is_rejected_without_renderer_fallback() {
    let original = EnvironmentTarget::exhibition()
        .resolve()
        .unwrap()
        .condition();
    for wind in [f64::NAN, f64::INFINITY, 61.0] {
        let mut condition = original;
        condition.waves.wind_velocity_ne_mps[0] = wind;
        assert!(WaterMaterial::from_environment(condition, false).is_err());
    }
    for fetch in [0.0, -1.0, 50_001.0, f64::NAN] {
        let mut condition = original;
        condition.waves.fetch_m = fetch;
        assert!(WaterMaterial::from_environment(condition, false).is_err());
    }
    for amplitude in [0.0, -1.0, 3.01, f64::NAN] {
        let mut condition = original;
        condition.waves.detail_amplitude_scale = amplitude;
        assert!(WaterMaterial::from_environment(condition, false).is_err());
    }
}

#[test]
fn patch_tracking_preserves_uploads_and_world_time() {
    let mut material = WaterMaterial::registered(false).unwrap();
    let waves = material.geometry_waves;
    let timing = material.geometry_motion;
    let bounds = material.near_bounds();
    material.project_camera(Vec3::new(-0.01, 10.0, 0.24), 3.0);
    assert_eq!(
        patch_center(material.camera_time.truncate()),
        Vec2::new(-1.0, 0.0)
    );
    assert_eq!(
        material.geometry_patch.truncate().truncate(),
        Vec2::new(-1.0, 0.0)
    );
    material.project_camera(Vec3::new(1.01, 20.0, 2.04), 3.0);
    assert_eq!(
        material.geometry_patch.truncate().truncate(),
        Vec2::new(1.0, 2.0)
    );
    assert_eq!(material.camera_time.w, 3.0);
    assert_eq!(material.geometry_waves, waves);
    assert_eq!(material.geometry_motion, timing);
    assert_eq!(material.near_bounds().half_extents, bounds.half_extents);
    assert_eq!(material.far_surface().geometry_patch, Vec4::ZERO);
}

#[test]
fn native_detail_uploads_keep_shared_rgba_and_linear_periodic_sampling() {
    let material = WaterMaterial::registered(false).unwrap();
    let configs = production_lake_detail_configs(material.visual_condition).unwrap();
    let images = prepare_detail_images(material.visual_condition).unwrap();
    for (image, config) in images.iter().zip(configs) {
        let reference = generate_lake_detail(config).unwrap();
        assert_eq!(
            &image.data.as_ref().unwrap()[..reference.rgba().len()],
            reference.rgba()
        );
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(image.texture_descriptor.mip_level_count, 10);
        assert!(
            image
                .texture_descriptor
                .usage
                .contains(TextureUsages::STORAGE_BINDING)
        );
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("Expected explicit sampler");
        };
        assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(sampler.address_mode_v, ImageAddressMode::Repeat);
        assert_eq!(sampler.anisotropy_clamp, 8);
        assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
        assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
    }
}

#[test]
fn native_sky_upload_preserves_web_srgb_radiance_and_panorama_orientation() {
    let reference = generate_lake_sky(None);
    let image = prepare_sky_image(None).unwrap();
    assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba16Float);
    assert_eq!(image.texture_descriptor.size.width, reference.width());
    assert_eq!(image.texture_descriptor.size.height, reference.height());
    assert_eq!(image.texture_descriptor.mip_level_count, 11);
    assert!(
        image
            .texture_descriptor
            .usage
            .contains(TextureUsages::STORAGE_BINDING)
    );
    for (column, row) in [(0, 0), (512, 256), (1023, 511)] {
        let offset = ((row * reference.width() + column) * 4) as usize;
        let pixel = &reference.rgba()[offset..offset + 4];
        let expected = Color::srgb_u8(pixel[0], pixel[1], pixel[2])
            .to_linear()
            .to_vec4();
        let actual = image
            .get_color_at(column, row)
            .unwrap()
            .to_linear()
            .to_vec4();
        assert!((actual - expected).abs().max_element() <= 0.0005);
        assert_eq!(actual.w, 1.0);
    }
    let ImageSampler::Descriptor(sampler) = &image.sampler else {
        panic!("Expected explicit panorama sampler");
    };
    assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
    assert_eq!(sampler.address_mode_v, ImageAddressMode::ClampToEdge);
    assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
    assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
}

#[test]
fn haze_and_daylight_follow_sealed_sky_and_preserve_missing_metadata() {
    let mut condition = EnvironmentTarget::exhibition()
        .resolve()
        .unwrap()
        .condition();
    let mut sky = condition.sky.unwrap();
    sky.visibility_m = 10_000.0;
    for (elevation, daylight) in [(0.0, 1.0), (55.0, 1.0), (-0.1, 0.0)] {
        sky.sun_elevation_degrees = elevation;
        sky.cloud_fraction = 0.95;
        condition.sky = Some(sky);
        let water = WaterMaterial::from_environment(condition, false).unwrap();
        assert_eq!(
            water.haze_parameters,
            Vec4::new(1750.0, 10_000.0, daylight, 0.0)
        );
        assert_eq!(water.far_surface().haze_parameters, water.haze_parameters);
        assert_eq!(
            WaterMaterial::from_environment(condition, true)
                .unwrap()
                .water_palette,
            water.water_palette
        );
    }
    condition.sky = None;
    let missing = WaterMaterial::from_environment(condition, false).unwrap();
    assert_eq!(
        missing.haze_parameters,
        Vec4::new(14_000.0, 80_000.0, 0.0, 0.0)
    );
    assert_eq!(missing.sun_cloud, Vec4::new(0.0, 1.0, 0.0, 0.0));
    condition.sky = Some(sky);
    for visibility in [
        0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::MAX,
        f64::MIN_POSITIVE,
    ] {
        condition.sky.as_mut().unwrap().visibility_m = visibility;
        assert!(WaterMaterial::from_environment(condition, false).is_err());
    }
}

#[test]
fn registered_water_and_sky_share_environment_without_transparency() {
    let water = WaterMaterial::registered(false).unwrap();
    let sky = WaterMaterial::registered(true).unwrap();
    let environment = bundled_environment().unwrap();
    let waves = &environment.document().waves;
    assert_eq!(water.camera_time, Vec4::ZERO);
    assert_eq!(sky.camera_time, water.camera_time);
    assert_eq!(sky.sun_cloud, water.sun_cloud);
    assert_eq!(sky.water_palette, water.water_palette);
    assert_eq!(sky.haze_parameters, water.haze_parameters);
    assert_eq!(sky.waves_sky.truncate(), water.waves_sky.truncate());
    assert_eq!(water.waves_sky.w, 0.0);
    assert_eq!(sky.waves_sky.w, 1.0);
    assert_eq!(water.waves_sky.x, waves.wind_velocity_ne_mps[1] as f32);
    assert_eq!(water.waves_sky.y, -waves.wind_velocity_ne_mps[0] as f32);
    assert_eq!(water.waves_sky.z, waves.detail_amplitude_scale as f32);
    assert!(water.sun_cloud.is_finite());
    assert!((water.sun_cloud.truncate().length() - 1.0).abs() < 1.0e-6);
    assert!(
        water
            .sun_cloud
            .truncate()
            .distance(Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78))
            < 1.0e-6
    );
    assert!(matches!(water.alpha_mode(), AlphaMode::Opaque));
    assert!(matches!(sky.alpha_mode(), AlphaMode::Opaque));
}
