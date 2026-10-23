//! Deterministic indigo wing pixels, registered alpha composition and linear-light mipmaps.

use image::{DynamicImage, GrayImage, RgbaImage, imageops, metadata::Cicp};

/// Width of each half-wing texture.
pub const WIDTH: u32 = 1024;
/// Height of each half-wing texture.
pub const HEIGHT: u32 = 256;
/// Registered decal width before cropping.
pub const DECAL_WIDTH: u32 = 2171;
/// Registered decal height before cropping.
pub const DECAL_HEIGHT: u32 = 724;
/// Number of RGBA bytes per side.
pub const SIDE_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;

/// Rejected image input or unavailable colour-space conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WingTextureError {
    /// Input pixels do not describe the fixed texture dimensions.
    InvalidPixels,
    /// Decal alpha does not describe the registered source image.
    InvalidDecal,
    /// The image library rejected the fixed sRGB conversion.
    ColorConversion,
}

/// Returns left then right opaque sRGB RGBA pixels, before the optional decal.
pub fn indigo_wing_textures() -> Vec<u8> {
    let mut pixels = vec![0; SIDE_BYTES * 2];
    for (side_index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let centers = if side < 0.0 {
            [0.08, 0.30, 0.47, 0.74, 0.89]
        } else {
            [0.09, 0.26, 0.54, 0.69, 0.92]
        };
        for row in 0..HEIGHT {
            let chord = (f64::from(row) + 0.5) / f64::from(HEIGHT);
            for column in 0..WIDTH {
                let span = (f64::from(column) + 0.5) / f64::from(WIDTH);
                let grain =
                    (f64::from(column) * 12.9898 + f64::from(row) * 78.233).sin() * 43758.5453;
                let weave = (grain - grain.floor() - 0.5) * 11.0;
                let dye = 7.0 * (span * 26.0 + chord * 9.0 + side).sin()
                    + 4.0 * (span * 87.0 - chord * 23.0).sin();
                let mut white = 0.0_f64;
                for (ring, center) in centers.into_iter().enumerate() {
                    let ring = ring as f64;
                    let along = (span - center) / (0.073 + 0.016 * (ring * 2.2 + side).sin());
                    let across = (chord - 0.26 - 0.055 * (ring * 1.7 + side).sin())
                        / (0.22 - 0.025 * (ring * 1.4).sin());
                    let angle = across.atan2(along);
                    let radius = along.hypot(across)
                        + 0.22 * (angle * 3.0 + ring * 1.3).sin()
                        + 0.13 * (angle * 7.0 - ring * 0.7).sin()
                        + 0.07 * (angle * 15.0 + span * 83.0).sin();
                    let rim_width = 0.22 + 0.09 * (angle * 5.0 + ring * 2.1).sin();
                    white = white.max(
                        smooth(0.80 - rim_width, 0.80, radius)
                            * (1.0 - smooth(0.84 + rim_width, 1.00 + rim_width, radius)),
                    );
                }
                let stroke = (chord
                    - 0.39
                    - 0.055 * (span * 19.0 + side * 0.8).sin()
                    - 0.025 * (span * 47.0 - side).sin())
                .abs();
                let brush_width = 0.02 + 0.01 * (span * 37.0 + side).sin();
                white = white.max(
                    (1.0 - smooth(brush_width, brush_width + 0.025, stroke))
                        * (1.0 - smooth(0.62, 0.82, span)),
                );
                white *= 0.82
                    + 0.18
                        * (span * 147.0 + chord * 71.0).sin()
                        * (chord * 49.0 - span * 59.0).sin();
                let index = side_index * SIDE_BYTES + (row * WIDTH + column) as usize * 4;
                for (channel, (indigo, resist)) in [(42.0, 218.0), (63.0, 224.0), (86.0, 218.0)]
                    .into_iter()
                    .enumerate()
                {
                    pixels[index + channel] = ((indigo + dye + weave) * (1.0 - white)
                        + (resist + weave * 0.4) * white)
                        .round() as u8;
                }
                pixels[index + 3] = 255;
            }
        }
    }
    pixels
}

fn smooth(from: f64, to: f64, value: f64) -> f64 {
    let fraction = ((value - from) / (to - from)).clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// Applies the registered source alpha once to both sides, preserving the lower half.
pub fn apply_indigo_wing_decal(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    alpha: &[u8],
) -> Result<(), WingTextureError> {
    if pixels.len() != SIDE_BYTES * 2 {
        return Err(WingTextureError::InvalidPixels);
    }
    if width != DECAL_WIDTH || height != DECAL_HEIGHT || alpha.len() != (width * height) as usize {
        return Err(WingTextureError::InvalidDecal);
    }
    let source =
        GrayImage::from_raw(width, height, alpha.to_vec()).ok_or(WingTextureError::InvalidDecal)?;
    for (side_index, crop_left) in [32, 1085].into_iter().enumerate() {
        let crop = imageops::crop_imm(&source, crop_left, 280, 1051, 148).to_image();
        let mut strip = imageops::resize(&crop, WIDTH, 66, imageops::FilterType::Triangle);
        if side_index == 0 {
            imageops::flip_horizontal_in_place(&mut strip);
        }
        for row in 0..HEIGHT / 2 {
            for column in 0..WIDTH {
                let opacity = if (31..97).contains(&row) {
                    f64::from(strip.get_pixel(column, row - 31)[0]) / 255.0
                } else {
                    0.0
                };
                let pixel = (row * WIDTH + column) as usize;
                let grain = ((pixel as f64 * 0.61803398875) % 1.0 - 0.5) * 8.0;
                let index = side_index * SIDE_BYTES + pixel * 4;
                for (channel, (indigo, resist)) in [(42.0, 220.0), (63.0, 224.0), (86.0, 218.0)]
                    .into_iter()
                    .enumerate()
                {
                    pixels[index + channel] =
                        ((indigo + grain) * (1.0 - opacity) + resist * opacity).round() as u8;
                }
            }
        }
    }
    Ok(())
}

/// Builds a complete sRGB upload chain with linear-light filtering, base level first.
pub fn indigo_wing_mipmaps(pixels: &[u8]) -> Result<Vec<u8>, WingTextureError> {
    let base = RgbaImage::from_raw(WIDTH, HEIGHT, pixels.to_vec())
        .ok_or(WingTextureError::InvalidPixels)?;
    if pixels.len() != SIDE_BYTES {
        return Err(WingTextureError::InvalidPixels);
    }
    let mut linear = DynamicImage::ImageRgba8(base).into_rgba32f();
    linear
        .apply_color_space(Cicp::SRGB_LINEAR, Default::default())
        .map_err(|_| WingTextureError::ColorConversion)?;
    let mut output = pixels.to_vec();
    for level in 1..=WIDTH.ilog2() {
        linear = imageops::resize(
            &linear,
            (WIDTH >> level).max(1),
            (HEIGHT >> level).max(1),
            imageops::FilterType::Triangle,
        );
        let mut encoded = linear.clone();
        encoded
            .apply_color_space(Cicp::SRGB, Default::default())
            .map_err(|_| WingTextureError::ColorConversion)?;
        output.extend(DynamicImage::ImageRgba32F(encoded).into_rgba8().into_raw());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_pixels_match_frozen_web_9435bc2_samples() {
        let pixels = indigo_wing_textures();
        for (side, expected) in [
            [
                [31, 52, 75],
                [215, 221, 216],
                [47, 68, 91],
                [39, 60, 83],
                [36, 57, 80],
                [39, 60, 83],
                [35, 56, 79],
            ],
            [
                [42, 63, 86],
                [190, 199, 197],
                [104, 119, 132],
                [93, 109, 122],
                [47, 68, 91],
                [45, 66, 89],
                [46, 67, 90],
            ],
        ]
        .into_iter()
        .enumerate()
        {
            for ((column, row), expected) in [
                (0, 0),
                (80, 32),
                (310, 70),
                (512, 96),
                (800, 127),
                (1000, 200),
                (123, 255),
            ]
            .into_iter()
            .zip(expected)
            {
                let start = side * SIDE_BYTES + (row * WIDTH as usize + column) * 4;
                for (actual, expected) in pixels[start..start + 3].iter().zip(expected) {
                    assert!(actual.abs_diff(expected) <= 1);
                }
            }
        }
        assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn decal_crops_mirrors_and_preserves_pixels_outside_the_upper_half() {
        let mut pixels = vec![19; SIDE_BYTES * 2];
        let mut alpha = vec![0; (DECAL_WIDTH * DECAL_HEIGHT) as usize];
        for row in 280..428 {
            for column in 32..558 {
                alpha[(row * DECAL_WIDTH + column) as usize] = 255;
            }
            for column in 1085..1611 {
                alpha[(row * DECAL_WIDTH + column) as usize] = 255;
            }
        }
        apply_indigo_wing_decal(&mut pixels, DECAL_WIDTH, DECAL_HEIGHT, &alpha).unwrap();
        for (side, white_column, dark_column) in [(0, 900, 100), (1, 100, 900)] {
            let index = |column, row| side * SIDE_BYTES + (row * WIDTH as usize + column) * 4;
            assert_eq!(
                &pixels[index(white_column, 50)..index(white_column, 50) + 3],
                &[220, 224, 218]
            );
            assert!(pixels[index(dark_column, 50)] < 50);
            assert!(pixels[index(white_column, 30)] < 50);
            assert!(pixels[index(white_column, 97)] < 50);
            assert!(
                pixels[side * SIDE_BYTES + SIDE_BYTES / 2..(side + 1) * SIDE_BYTES]
                    .iter()
                    .all(|pixel| *pixel == 19)
            );
        }
        let retained = pixels.clone();
        assert_eq!(
            apply_indigo_wing_decal(&mut pixels, 1, 1, &[255]),
            Err(WingTextureError::InvalidDecal)
        );
        assert_eq!(pixels, retained);
    }

    #[test]
    fn mipmaps_keep_the_base_and_filter_black_and_white_in_linear_light() {
        let mut pixels = vec![255; SIDE_BYTES];
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            if (index % WIDTH as usize).is_multiple_of(2) {
                pixel[..3].fill(0);
            }
        }
        let mipmaps = indigo_wing_mipmaps(&pixels).unwrap();
        let size: usize = (0..=WIDTH.ilog2())
            .map(|level| (WIDTH >> level).max(1) as usize * (HEIGHT >> level).max(1) as usize * 4)
            .sum();
        assert_eq!(mipmaps.len(), size);
        assert_eq!(&mipmaps[..SIDE_BYTES], pixels);
        assert!((185..=190).contains(&mipmaps[mipmaps.len() - 4]));
        assert_eq!(mipmaps.last(), Some(&255));
    }
}
