use anyhow::Result;
use wasm_bindgen::prelude::*;
use yew::Renderer;

use pixel_filter::layout::*;

fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    Renderer::<App>::new().render();
    Ok(())
}
