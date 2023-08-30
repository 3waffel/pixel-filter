use anyhow::Result;
use image::{ImageBuffer, ImageError, Rgba};
use palette::{color_difference::EuclideanDistance, IntoColor, Oklab, Srgb};
use std::rc::Rc;
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;

// const INPUT_PATH: &str = "images/test.png";
// const OUTPUT_PATH: &str = "images/output.png";

const THRESHOLD_MAP: [[usize; 2]; 2] = [[0, 2], [3, 1]];
const MAP_SIZE: usize = THRESHOLD_MAP.len();
const COLOR_DITHER: f32 = 0.04;
const ALPHA_DITHER: f32 = 0.12;

const PALETTE_HEX: [&str; 48] = [
    "1b112c", "413047", "543e54", "75596f", "91718b", "b391aa", "ccb3c6", "e3cfe3", "fff7ff",
    "fffbb5", "faf38e", "f7d076", "fa9c69", "eb7363", "e84545", "c22e53", "943054", "612147",
    "3d173c", "3f233c", "66334b", "8c4b63", "c16a7d", "e5959f", "ffccd0", "dd8d9f", "c8658d",
    "b63f82", "9e2083", "731f7a", "47195d", "2a143d", "183042", "1e5451", "2a6957", "3b804d",
    "5aa653", "86cf74", "caf095", "e0f0bd", "3f275e", "3f317a", "3c548f", "456aa1", "4a84b0",
    "56aec4", "92d7d9", "c3ebe3",
];

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let target = document
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let target_context = target
        .get_context("2d")?
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    let origin = document
        .create_element("canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    origin.set_id("origin");
    let container = document.get_element_by_id("origin-container").unwrap();
    container.append_child(&origin)?;
    let origin_context = origin
        .get_context("2d")?
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    let img = Rc::new(
        document
            .get_element_by_id("img")
            .unwrap()
            .dyn_into::<web_sys::HtmlImageElement>()?,
    );
    let img_cb = img.clone();

    let callback = Closure::<dyn Fn()>::new(move || {
        origin.set_width(img_cb.natural_width());
        origin.set_height(img_cb.natural_height());
        origin_context
            .draw_image_with_html_image_element(&img_cb, 0.0, 0.0)
            .unwrap();

        let data = origin_context
            .get_image_data(0.0, 0.0, origin.width() as f64, origin.height() as f64)
            .unwrap();
        let raw_data = data.data().0;
        let converted: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(img_cb.natural_width(), img_cb.natural_height(), raw_data)
                .unwrap();

        let buf = run(converted).unwrap();
        let clamped_buf: Clamped<&[u8]> = Clamped(buf.as_raw());
        let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(
            clamped_buf,
            img_cb.natural_width(),
            img_cb.natural_height(),
        )
        .unwrap();
        target.set_width(img_cb.natural_width());
        target.set_height(img_cb.natural_height());
        target_context
            .put_image_data(&image_data_temp, 0.0, 0.0)
            .unwrap();
    });

    img.add_event_listener_with_callback("load", callback.as_ref().unchecked_ref())?;

    callback.forget();
    Ok(())
}

fn run(img: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, ImageError> {
    // let img = Reader::open(INPUT_PATH)?.decode()?;
    // let pixels = img.as_rgba8().unwrap().enumerate_pixels();
    let pixels = img.enumerate_pixels();
    let mut output_buffer = ImageBuffer::<Rgba<u8>, _>::new(img.width(), img.height());

    let palette_oklab = palette_as_oklab();
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
    // output_buffer.save(OUTPUT_PATH)?;

    Ok(output_buffer)
}

fn palette_as_oklab() -> Vec<Oklab> {
    let mut oklab_palette: Vec<Oklab> = vec![];
    for hex in PALETTE_HEX {
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
