use std::env;
use std::fs;
use std::path::PathBuf;

use gorillas::play_ref::{canonical_play_sequences, sound_from_play_sequence};
use serde::Serialize;

#[derive(Serialize)]
struct WavManifestEntry {
    name: &'static str,
    sequence: &'static str,
    volume: f32,
}

fn runtime_volume(name: &str) -> f32 {
    match name {
        "throw" => 0.30,
        "building_explosion" | "gorilla_explosion" => 0.40,
        _ => 0.28,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: cargo run --bin gorillas_play_wav -- <output-dir>")?;

    fs::create_dir_all(&output_dir)?;

    let manifest = canonical_play_sequences()
        .iter()
        .map(|entry| {
            let volume = runtime_volume(entry.name);
            let wav_bytes = sound_from_play_sequence(entry.sequence, volume)?;
            fs::write(output_dir.join(format!("{}.wav", entry.name)), wav_bytes)?;
            Ok(WavManifestEntry {
                name: entry.name,
                sequence: entry.sequence,
                volume,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

    fs::write(
        output_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    Ok(())
}
