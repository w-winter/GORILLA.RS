use std::env;
use std::fs;
use std::path::PathBuf;

use gorillas::render_ref::{palette_rgb, render_checkpoints, RenderCheckpoint};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(
        env::args()
            .nth(1)
            .ok_or("usage: cargo run --bin gorillas_render_trace -- <output-dir>")?,
    );
    fs::create_dir_all(&output_dir)?;

    let checkpoints = render_checkpoints();

    for checkpoint in &checkpoints {
        let ppm_path = output_dir.join(format!("{}.ppm", checkpoint.name));
        fs::write(ppm_path, ppm_bytes(checkpoint))?;
    }

    let manifest = serde_json::to_string_pretty(&checkpoints)? + "\n";
    fs::write(output_dir.join("render_checkpoints.json"), manifest)?;

    Ok(())
}

fn ppm_bytes(checkpoint: &RenderCheckpoint) -> Vec<u8> {
    let mut payload = Vec::<u8>::new();
    payload.extend_from_slice(
        format!("P6\n{} {}\n255\n", checkpoint.width, checkpoint.height).as_bytes(),
    );

    for &pixel in &checkpoint.pixels {
        payload.extend_from_slice(&palette_rgb(pixel));
    }

    payload
}
