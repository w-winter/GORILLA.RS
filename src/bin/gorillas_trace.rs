use std::env;
use std::fs;
use std::path::PathBuf;

use gorillas::{simulate_trace_scenario, TraceScenario};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input_path = PathBuf::from(
        args.next()
            .ok_or("usage: cargo run --bin gorillas_trace -- <scenario.json> [output.json]")?,
    );
    let output_path = args.next().map(PathBuf::from);

    let scenario_text = fs::read_to_string(&input_path)?;
    let scenario: TraceScenario = serde_json::from_str(&scenario_text)?;
    let trace = simulate_trace_scenario(&scenario)?;
    let trace_json = serde_json::to_string_pretty(&trace)? + "\n";

    if let Some(path) = output_path {
        fs::write(path, trace_json)?;
    } else {
        print!("{}", trace_json);
    }

    Ok(())
}
