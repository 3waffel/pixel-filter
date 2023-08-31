use anyhow::Result;
use image::io::Reader;
use pixel_filter::filter::*;

const INPUT_PATH: &str = "images/test.png";
const OUTPUT_PATH: &str = "images/output.png";

fn main() -> Result<()> {
    let img = Reader::open(INPUT_PATH)?.decode()?;
    let output_buffer = run(img.as_rgba8().unwrap())?;
    output_buffer.save(OUTPUT_PATH)?;
    Ok(())
}
