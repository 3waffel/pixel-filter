use super::filter::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use gloo::file::{
    callbacks::{read_as_bytes, FileReader},
    File,
};
use image::{ImageBuffer, Rgba};
use js_sys::Math::random;
use std::collections::HashMap;
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{
    CanvasRenderingContext2d, Event, FileList, HtmlCanvasElement, HtmlImageElement,
    HtmlInputElement, ImageData,
};
use yew::prelude::*;

#[allow(unused)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

pub enum Msg {
    Filter,
    Files(Option<FileList>),
    Loaded(String, String, Vec<u8>),
    Random,
    OnEdit(String, String),
}

#[derive(Default)]
pub struct App {
    threshold_map: [[usize; 2]; 2],
    color_dither: f32,
    alpha_dither: f32,
    palette_hex: Vec<String>,

    image_element: NodeRef,
    target_canvas: NodeRef,
    readers: HashMap<String, FileReader>,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            threshold_map: THRESHOLD_MAP,
            color_dither: COLOR_DITHER,
            alpha_dither: ALPHA_DITHER,
            palette_hex: PALETTE_HEX.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loaded(file_name, file_type, data) => {
                let image_element = self.image_element.cast::<HtmlImageElement>().unwrap();

                image_element.set_src(&format!(
                    "data:{};base64,{}",
                    file_type,
                    STANDARD.encode(&data)
                ));
                self.readers.remove(&file_name);
                true
            }
            Msg::Files(files) => {
                match files {
                    Some(files) => {
                        let files = js_sys::try_iter(&files)
                            .unwrap()
                            .unwrap()
                            .map(|v| web_sys::File::from(v.unwrap()))
                            .map(File::from)
                            .collect::<Vec<_>>();

                        if files.len() != 0 {
                            let link = ctx.link().clone();
                            let file = files[0].clone();
                            self.readers.insert(
                                file.name(),
                                read_as_bytes(&file.clone(), move |res| {
                                    link.send_message(Msg::Loaded(
                                        file.name(),
                                        file.raw_mime_type(),
                                        res.expect("Failed to read file"),
                                    ))
                                }),
                            );
                        }
                    }
                    None => {}
                }
                true
            }
            Msg::Filter => {
                let image_element = self.image_element.cast::<HtmlImageElement>().unwrap();
                let target_canvas = self.target_canvas.cast::<HtmlCanvasElement>().unwrap();
                let target_context = target_canvas
                    .get_context("2d")
                    .unwrap()
                    .unwrap()
                    .dyn_into::<CanvasRenderingContext2d>()
                    .unwrap();

                target_canvas.set_width(image_element.natural_width());
                target_canvas.set_height(image_element.natural_height());
                target_context
                    .draw_image_with_html_image_element(&image_element, 0.0, 0.0)
                    .unwrap();

                let data = target_context
                    .get_image_data(
                        0.0,
                        0.0,
                        target_canvas.width() as f64,
                        target_canvas.height() as f64,
                    )
                    .unwrap();
                let raw_data = data.data().0;
                let converted: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(
                    image_element.natural_width(),
                    image_element.natural_height(),
                    raw_data,
                )
                .unwrap();

                // run filter
                let buf = run_with_parameters(
                    &converted,
                    &self.threshold_map,
                    self.color_dither,
                    self.alpha_dither,
                    self.palette_hex
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .unwrap();
                let clamped_buf: Clamped<&[u8]> = Clamped(buf.as_raw());
                let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(
                    clamped_buf,
                    image_element.natural_width(),
                    image_element.natural_height(),
                )
                .unwrap();
                target_canvas.set_width(image_element.natural_width());
                target_canvas.set_height(image_element.natural_height());
                target_context
                    .put_image_data(&image_data_temp, 0.0, 0.0)
                    .unwrap();
                true
            }
            Msg::Random => {
                let image_element = self.image_element.cast::<HtmlImageElement>().unwrap();
                let seed = (random() * 50.).floor() as usize;
                image_element.set_src(&format!(
                    "https://source.unsplash.com/random/100x100/?{}",
                    seed
                ));
                true
            }
            Msg::OnEdit(id, value) => {
                match id.as_str() {
                    "color_dither" => match value.parse() {
                        Ok(s) => self.color_dither = s,
                        Err(_) => return false,
                    },
                    "alpha_dither" => match value.parse() {
                        Ok(s) => self.alpha_dither = s,
                        Err(_) => return false,
                    },
                    "threshold_map" => match serde_json::from_str(&value) {
                        Ok(s) => self.threshold_map = s,
                        Err(_) => return false,
                    },
                    "palette_hex" => match serde_json::from_str(&value) {
                        Ok(s) => self.palette_hex = s,
                        Err(_) => return false,
                    },
                    _ => {}
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <>
                <h1>{"Pixel Filter"}</h1>
                <section class="image-display">
                <div class="origin">
                    <h3>{"Original Image"}</h3>
                    <img id="img" width="224px" crossorigin="anonymous"
                    ref={self.image_element.clone()} />
                    <input
                    id="img-input"
                    type="file"
                    accept="image/png, image/jpeg"
                    onchange={ctx.link().callback(|e: Event| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Msg::Files(input.files())
                    })}
                    />
                    <div>
                        <button onclick={ctx.link().callback(|_| {
                            Msg::Random
                        }) }>{ "Random Image" }</button>
                        <button onclick={ctx.link().callback(|_| Msg::Filter)}>{ "Filter" }</button>
                    </div>
                </div>

                <div class="filtered">
                    <h3>{"Filtered Canvas"}</h3>
                    <canvas id="canvas" width="224" ref={self.target_canvas.clone()}></canvas>
                </div>

                <div class="parameters">
                    <h3>{"Parameters"}</h3>
                    <label for="threshold_map">{ "Threshold Map" }</label>
                    <input
                        type="text"
                        id="threshold_map"
                        value={ format!("{:?}", &self.threshold_map) }
                        onchange={ctx.link().callback(|e: Event| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            Msg::OnEdit(input.id(), input.value())
                        })}
                        />

                    <label for="color_dither">{ "Color Dither" }</label>
                    <input
                        type="range"
                        min="0"
                        max="1"
                        step="any"
                        id="color_dither"
                        value={ format!("{}", &self.color_dither) }
                        onchange={ctx.link().callback(|e: Event| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            Msg::OnEdit(input.id(), input.value())
                        })}
                        />

                    <label for="alpha_dither">{ "Alpha Dither" }</label>
                    <input
                        type="range"
                        min="0"
                        max="1"
                        step="any"
                        id="alpha_dither"
                        value={ format!("{}", &self.alpha_dither) }
                        onchange={ctx.link().callback(|e: Event| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            Msg::OnEdit(input.id(), input.value())
                        })}
                        />

                    <label for="palette_hex">{ "Palette Hex" }</label>
                    <textarea
                        type="text"
                        id="palette_hex"
                        value={ format!("{:?}", &self.palette_hex) }
                        onchange={ctx.link().callback(|e: Event| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            Msg::OnEdit(input.id(), input.value())
                        })}
                        />
                </div>
                </section>
            </>
        }
    }
}
