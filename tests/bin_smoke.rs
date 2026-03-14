use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("gorillas-{label}-{timestamp}"))
}

fn assert_json_file(path: &Path) {
    let payload = fs::read_to_string(path).expect("expected JSON file");
    serde_json::from_str::<serde_json::Value>(&payload).expect("expected valid JSON payload");
}

fn assert_wav_file(path: &Path) {
    let payload = fs::read(path).expect("expected WAV file");
    assert!(
        payload.starts_with(b"RIFF"),
        "expected RIFF header in {path:?}"
    );
}

#[test]
fn test_gorillas_play_trace_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_gorillas_play_trace"))
        .output()
        .expect("failed to execute gorillas_play_trace");

    assert!(
        output.status.success(),
        "gorillas_play_trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("gorillas_play_trace must emit valid JSON");
}

#[test]
fn test_gorillas_render_trace_smoke() {
    let output_dir = unique_temp_path("render-trace");

    let output = Command::new(env!("CARGO_BIN_EXE_gorillas_render_trace"))
        .arg(&output_dir)
        .output()
        .expect("failed to execute gorillas_render_trace");

    assert!(
        output.status.success(),
        "gorillas_render_trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_json_file(&output_dir.join("render_checkpoints.json"));

    fs::remove_dir_all(output_dir).expect("failed to clean render trace temp dir");
}

#[test]
fn test_gorillas_scene_trace_smoke() {
    let output_dir = unique_temp_path("scene-trace");

    let output = Command::new(env!("CARGO_BIN_EXE_gorillas_scene_trace"))
        .arg(&output_dir)
        .output()
        .expect("failed to execute gorillas_scene_trace");

    assert!(
        output.status.success(),
        "gorillas_scene_trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_json_file(&output_dir.join("scene_checkpoints.json"));

    fs::remove_dir_all(output_dir).expect("failed to clean scene trace temp dir");
}

#[test]
fn test_gorillas_trace_smoke() {
    let output_file = unique_temp_path("trace").with_extension("json");
    let scenario_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/sample_trace_scenario.json");

    let output = Command::new(env!("CARGO_BIN_EXE_gorillas_trace"))
        .arg(scenario_path)
        .arg(&output_file)
        .output()
        .expect("failed to execute gorillas_trace");

    assert!(
        output.status.success(),
        "gorillas_trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_json_file(&output_file);

    fs::remove_file(output_file).expect("failed to clean trace output file");
}

#[test]
fn test_gorillas_play_wav_smoke() {
    let output_dir = unique_temp_path("play-wav");

    let output = Command::new(env!("CARGO_BIN_EXE_gorillas_play_wav"))
        .arg(&output_dir)
        .output()
        .expect("failed to execute gorillas_play_wav");

    assert!(
        output.status.success(),
        "gorillas_play_wav failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_json_file(&output_dir.join("manifest.json"));
    assert_wav_file(&output_dir.join("throw.wav"));

    fs::remove_dir_all(output_dir).expect("failed to clean play wav temp dir");
}
