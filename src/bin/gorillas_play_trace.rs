use gorillas::play_ref::{canonical_play_sequences, trace_sequence};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let traces = canonical_play_sequences()
        .iter()
        .map(|entry| trace_sequence(entry.name, entry.sequence))
        .collect::<Result<Vec<_>, _>>()?;

    print!("{}", serde_json::to_string_pretty(&traces)?);
    println!();
    Ok(())
}
