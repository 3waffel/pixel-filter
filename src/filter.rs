use anyhow::Result;
use image::{ImageBuffer, ImageError, Rgba};
use palette::{color_difference::EuclideanDistance, IntoColor, Oklab, Srgb};

pub const THRESHOLD_MAP: [[usize; 2]; 2] = [[0, 2], [3, 1]];
pub const MAP_SIZE: usize = THRESHOLD_MAP.len();
pub const COLOR_DITHER: f32 = 0.04;
pub const ALPHA_DITHER: f32 = 0.12;

pub const PALETTE_HEX: [&str; 48] = [
    "1b112c", "413047", "543e54", "75596f", "91718b", "b391aa", "ccb3c6", "e3cfe3", "fff7ff",
    "fffbb5", "faf38e", "f7d076", "fa9c69", "eb7363", "e84545", "c22e53", "943054", "612147",
    "3d173c", "3f233c", "66334b", "8c4b63", "c16a7d", "e5959f", "ffccd0", "dd8d9f", "c8658d",
    "b63f82", "9e2083", "731f7a", "47195d", "2a143d", "183042", "1e5451", "2a6957", "3b804d",
    "5aa653", "86cf74", "caf095", "e0f0bd", "3f275e", "3f317a", "3c548f", "456aa1", "4a84b0",
    "56aec4", "92d7d9", "c3ebe3",
];

pub fn run_with_parameters(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    threshold_map: &[[usize; 2]; 2],
    color_dither: f32,
    alpha_dither: f32,
    palette_hex: &[&str],
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, ImageError> {
    let map_size = threshold_map.len();
    let pixels = img.enumerate_pixels();
    let mut output_buffer = ImageBuffer::<Rgba<u8>, _>::new(img.width(), img.height());

    let palette_oklab = palette_as_oklab(palette_hex);
    for pixel in pixels {
        let (x, y) = (pixel.0, pixel.1);
        let [r, g, b, a] = pixel.2 .0;

        let alpha_f32 = (a as f32) / 255.0;
        let pixel_rgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let pixel_oklab: Oklab = pixel_rgb.into_color();

        // create a list of candidate color and alpha values
        let mut candidates_c: Vec<Oklab> = vec![];
        let mut candidates_a: Vec<f32> = vec![];
        let mut error_c = Oklab::new(0.0, 0.0, 0.0);
        let mut error_a = 0.0;
        for _ in 0..map_size.pow(2) {
            // color
            let sample_c = pixel_oklab + error_c * color_dither;
            let candidate_c = find_closest(&palette_oklab, sample_c);
            candidates_c.push(candidate_c);
            error_c += pixel_oklab - candidate_c;

            // alpha
            let sample_a = alpha_f32 + error_a * alpha_dither;
            let candidate_a = sample_a.round();
            candidates_a.push(candidate_a);
            error_a += alpha_f32 - candidate_a;
        }

        // sort candidates by brightness and alpha, respectively
        candidates_c
            .sort_by(|Oklab { l: l1, .. }, Oklab { l: l2, .. }| l1.partial_cmp(l2).unwrap());
        candidates_a.sort_by(|a1, a2| a1.partial_cmp(&a2).unwrap());

        // choose a candidate based on the pixel coordinates
        let index = threshold_map[x as usize % map_size][y as usize % map_size];
        let chosen_color: Srgb = candidates_c[index].into_color();
        let chosen_alpha = candidates_a[index];

        // output the new color to the buffer
        let output_pixel = output_buffer.get_pixel_mut(x, y);
        *output_pixel = image::Rgba([
            (chosen_color.red * 255.0) as u8,
            (chosen_color.green * 255.0) as u8,
            (chosen_color.blue * 255.0) as u8,
            (chosen_alpha * 255.0) as u8,
        ]);
    }

    Ok(output_buffer)
}

pub fn run(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, ImageError> {
    let pixels = img.enumerate_pixels();
    let mut output_buffer = ImageBuffer::<Rgba<u8>, _>::new(img.width(), img.height());

    let palette_oklab = palette_as_oklab(&PALETTE_HEX);
    for pixel in pixels {
        let (x, y) = (pixel.0, pixel.1);
        let [r, g, b, a] = pixel.2 .0;

        let alpha_f32 = (a as f32) / 255.0;
        let pixel_rgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let pixel_oklab: Oklab = pixel_rgb.into_color();

        // create a list of candidate color and alpha values
        let mut candidates_c: Vec<Oklab> = vec![];
        let mut candidates_a: Vec<f32> = vec![];
        let mut error_c = Oklab::new(0.0, 0.0, 0.0);
        let mut error_a = 0.0;
        for _ in 0..MAP_SIZE.pow(2) {
            // color
            let sample_c = pixel_oklab + error_c * COLOR_DITHER;
            let candidate_c = find_closest(&palette_oklab, sample_c);
            candidates_c.push(candidate_c);
            error_c += pixel_oklab - candidate_c;

            // alpha
            let sample_a = alpha_f32 + error_a * ALPHA_DITHER;
            let candidate_a = sample_a.round();
            candidates_a.push(candidate_a);
            error_a += alpha_f32 - candidate_a;
        }

        // sort candidates by brightness and alpha, respectively
        candidates_c
            .sort_by(|Oklab { l: l1, .. }, Oklab { l: l2, .. }| l1.partial_cmp(l2).unwrap());
        candidates_a.sort_by(|a1, a2| a1.partial_cmp(&a2).unwrap());

        // choose a candidate based on the pixel coordinates
        let index = THRESHOLD_MAP[x as usize % MAP_SIZE][y as usize % MAP_SIZE];
        let chosen_color: Srgb = candidates_c[index].into_color();
        let chosen_alpha = candidates_a[index];

        // output the new color to the buffer
        let output_pixel = output_buffer.get_pixel_mut(x, y);
        *output_pixel = image::Rgba([
            (chosen_color.red * 255.0) as u8,
            (chosen_color.green * 255.0) as u8,
            (chosen_color.blue * 255.0) as u8,
            (chosen_alpha * 255.0) as u8,
        ]);
    }

    Ok(output_buffer)
}

fn palette_as_oklab(palette_hex: &[&str]) -> Vec<Oklab> {
    let mut oklab_palette: Vec<Oklab> = vec![];
    for hex in palette_hex {
        let rgb = hex_to_rgb(hex).unwrap();
        oklab_palette.push(rgb.into_color());
    }
    oklab_palette
}

fn hex_to_rgb(hex: &str) -> Result<Srgb, &'static str> {
    if hex.len() != 6 {
        return Err("Invalid hex color code");
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color code")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color code")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color code")?;

    Ok(Srgb::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ))
}

fn find_closest(palette: &Vec<Oklab>, color: Oklab) -> Oklab {
    let mut dist_of_closest = std::f32::MAX;
    let mut closest = Oklab::new(0.0, 0.0, 0.0);

    for palette_color in palette {
        let d = color.distance_squared(*palette_color);
        if d < dist_of_closest {
            dist_of_closest = d;
            closest = *palette_color
        }
    }
    closest
}
