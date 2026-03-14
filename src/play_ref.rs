//! Reference QBASIC `PLAY` parsing, timing, synthesis, and WAV serialization
//!
//! The file stays intentionally contiguous so a reader can follow the full path
//! from a canonical sequence string to quantized PC-speaker-style output.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

// IBM PC PIT input clock: 14.31818 MHz colorburst crystal divided by 12
const PC_SPEAKER_CLOCK_HZ: f32 = 1_193_181.666_666_7;
const PC_SPEAKER_ATTACK_SECONDS: f32 = 0.0015;
// Keep note releases short so QBASIC PLAY cadences end crisply instead of smearing
const PC_SPEAKER_RELEASE_SECONDS: f32 = 0.0015;
const PC_SPEAKER_HIGHPASS_HZ: f32 = 500.0;
const PC_SPEAKER_LOWPASS_HZ: f32 = 2_500.0;
const PLAY_OCTAVE_MIDI_OFFSET: i32 = 3;
const MIN_PLAY_OCTAVE: i32 = 0;
const MAX_PLAY_OCTAVE: i32 = 6;
const MIN_NOTE_NUMBER: i32 = 1;
const MAX_NOTE_NUMBER: i32 = 84;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NoteMode {
    Staccato,
    Normal,
    Legato,
}

impl NoteMode {
    fn note_on_ratio(self) -> f32 {
        match self {
            Self::Staccato => 0.75,
            Self::Normal => 0.875,
            Self::Legato => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlayState {
    tempo: f32,
    octave: i32,
    default_length: u32,
    note_mode: NoteMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NamedPlaySequence {
    pub name: &'static str,
    pub sequence: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaySequenceTrace {
    pub name: String,
    pub sequence: String,
    pub events: Vec<PlayEvent>,
    pub total_duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PlayEvent {
    Note {
        note_index: usize,
        ideal_frequency_hz: f32,
        quantized_frequency_hz: f32,
        pit_divisor: u32,
        duration_seconds: f32,
        note_on_seconds: f32,
        note_off_seconds: f32,
        mode: NoteMode,
    },
    Rest {
        note_index: usize,
        duration_seconds: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayTraceError {
    UnknownSequenceName(String),
    UnsupportedToken {
        token: char,
        position: usize,
    },
    InvalidModeToken {
        token: char,
        position: usize,
    },
    MissingNumber {
        token: char,
        position: usize,
    },
    InvalidTempo {
        value: u32,
        position: usize,
    },
    InvalidLength {
        token: char,
        value: u32,
        position: usize,
    },
    OctaveOutOfRange {
        value: i32,
        position: usize,
    },
    NoteNumberOutOfRange {
        value: i32,
        position: usize,
    },
}

impl fmt::Display for PlayTraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSequenceName(name) => {
                write!(f, "unknown PLAY sequence name: {name}")
            }
            Self::UnsupportedToken { token, position } => {
                write!(f, "unsupported PLAY token '{token}' at position {position}")
            }
            Self::InvalidModeToken { token, position } => {
                write!(
                    f,
                    "unsupported PLAY mode token '{token}' at position {position}"
                )
            }
            Self::MissingNumber { token, position } => {
                write!(f, "missing number after '{token}' at position {position}")
            }
            Self::InvalidTempo { value, position } => {
                write!(
                    f,
                    "invalid tempo value {value} at position {position}; expected >= 1"
                )
            }
            Self::InvalidLength {
                token,
                value,
                position,
            } => {
                write!(
                    f,
                    "invalid length value {value} after '{token}' at position {position}; expected >= 1"
                )
            }
            Self::OctaveOutOfRange { value, position } => {
                write!(
                    f,
                    "octave value {value} out of range at position {position}; expected {MIN_PLAY_OCTAVE}..={MAX_PLAY_OCTAVE}"
                )
            }
            Self::NoteNumberOutOfRange { value, position } => {
                write!(
                    f,
                    "note number {value} out of range at position {position}; expected {MIN_NOTE_NUMBER}..={MAX_NOTE_NUMBER}"
                )
            }
        }
    }
}

impl Error for PlayTraceError {}

const CANONICAL_PLAY_SEQUENCES: &[NamedPlaySequence] = &[
    NamedPlaySequence {
        name: "building_explosion",
        sequence: "MBO0L32EFGEFDC",
    },
    NamedPlaySequence {
        name: "gorilla_explosion",
        sequence: "MBO0L16EFGEFDC",
    },
    NamedPlaySequence {
        name: "intro_riff_1",
        sequence: "t120o1l16b9n0baan0bn0bn0baaan0b9n0baan0b",
    },
    NamedPlaySequence {
        name: "intro_riff_2",
        sequence: "o2l16e-9n0e-d-d-n0e-n0e-n0e-d-d-d-n0e-9n0e-d-d-n0e-",
    },
    NamedPlaySequence {
        name: "intro_riff_3",
        sequence: "o2l16g-9n0g-een0g-n0g-n0g-eeen0g-9n0g-een0g-",
    },
    NamedPlaySequence {
        name: "intro_riff_4",
        sequence: "o2l16b9n0baan0g-n0g-n0g-eeen0o1b9n0baan0b",
    },
    NamedPlaySequence {
        name: "intro_fast_left",
        sequence: "T160O0L32EFGEFDC",
    },
    NamedPlaySequence {
        name: "intro_fast_right",
        sequence: "T160O0L32EFGEFDC",
    },
    NamedPlaySequence {
        name: "intro_theme",
        sequence: "MBT160O1L8CDEDCDL4ECC",
    },
    NamedPlaySequence {
        name: "throw",
        sequence: "MBo0L32A-L64CL16BL64A+",
    },
    NamedPlaySequence {
        name: "victory_dance_left",
        sequence: "MFO0L32EFGEFDC",
    },
    NamedPlaySequence {
        name: "victory_dance_right",
        sequence: "MFO0L32EFGEFDC",
    },
];

const RUNTIME_AUDIO_SEQUENCE_NAMES: &[&str] = &[
    "intro_theme",
    "throw",
    "building_explosion",
    "gorilla_explosion",
    "intro_fast_left",
    "victory_dance_left",
    "intro_riff_1",
    "intro_riff_2",
    "intro_riff_3",
    "intro_riff_4",
];

// Canonical catalog and public lookup helpers
pub fn canonical_play_sequences() -> &'static [NamedPlaySequence] {
    CANONICAL_PLAY_SEQUENCES
}

pub fn runtime_audio_sequence_names() -> &'static [&'static str] {
    RUNTIME_AUDIO_SEQUENCE_NAMES
}

pub fn canonical_play_sequence(name: &str) -> Result<&'static str, PlayTraceError> {
    CANONICAL_PLAY_SEQUENCES
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.sequence)
        .ok_or_else(|| PlayTraceError::UnknownSequenceName(name.to_string()))
}

pub fn trace_named_sequence(name: &str) -> Result<PlaySequenceTrace, PlayTraceError> {
    let sequence = canonical_play_sequence(name)?;
    trace_sequence(name, sequence)
}

// PLAY parser and trace emission
pub fn trace_sequence(name: &str, sequence: &str) -> Result<PlaySequenceTrace, PlayTraceError> {
    let bytes = sequence.as_bytes();
    let mut cursor = 0usize;
    let mut note_index = 0usize;
    let mut state = PlayState {
        tempo: 120.0,
        octave: 4,
        default_length: 4,
        note_mode: NoteMode::Normal,
    };
    let mut events = Vec::new();
    let mut total_duration = 0.0f32;

    while cursor < bytes.len() {
        let token_index = cursor;
        let token = bytes[cursor] as char;
        cursor += 1;

        match token.to_ascii_uppercase() {
            whitespace if whitespace.is_ascii_whitespace() => {}
            'M' => {
                if cursor >= bytes.len() {
                    return Err(PlayTraceError::MissingNumber {
                        token,
                        position: token_index,
                    });
                }

                let mode = bytes[cursor] as char;
                cursor += 1;

                match mode.to_ascii_uppercase() {
                    'S' => state.note_mode = NoteMode::Staccato,
                    'N' => state.note_mode = NoteMode::Normal,
                    'L' => state.note_mode = NoteMode::Legato,
                    'B' | 'F' => {}
                    _ => {
                        return Err(PlayTraceError::InvalidModeToken {
                            token: mode,
                            position: cursor - 1,
                        });
                    }
                }
            }
            'T' => {
                let (number, next_cursor) =
                    parse_required_number(bytes, cursor, token, token_index)?;
                if number == 0 {
                    return Err(PlayTraceError::InvalidTempo {
                        value: number,
                        position: token_index,
                    });
                }

                state.tempo = number as f32;
                cursor = next_cursor;
            }
            'O' => {
                let (number, next_cursor) =
                    parse_required_number(bytes, cursor, token, token_index)?;
                let octave = number as i32;
                if !(MIN_PLAY_OCTAVE..=MAX_PLAY_OCTAVE).contains(&octave) {
                    return Err(PlayTraceError::OctaveOutOfRange {
                        value: octave,
                        position: token_index,
                    });
                }

                state.octave = octave;
                cursor = next_cursor;
            }
            'L' => {
                let (number, next_cursor) =
                    parse_required_number(bytes, cursor, token, token_index)?;
                if number == 0 {
                    return Err(PlayTraceError::InvalidLength {
                        token,
                        value: number,
                        position: token_index,
                    });
                }

                state.default_length = number;
                cursor = next_cursor;
            }
            '<' => {
                let next = state.octave - 1;
                if next < MIN_PLAY_OCTAVE {
                    return Err(PlayTraceError::OctaveOutOfRange {
                        value: next,
                        position: token_index,
                    });
                }

                state.octave = next;
            }
            '>' => {
                let next = state.octave + 1;
                if next > MAX_PLAY_OCTAVE {
                    return Err(PlayTraceError::OctaveOutOfRange {
                        value: next,
                        position: token_index,
                    });
                }

                state.octave = next;
            }
            'P' => {
                let (length, next_cursor) = parse_length_or_default(
                    bytes,
                    cursor,
                    state.default_length,
                    token,
                    token_index,
                )?;
                let dots = count_dots(bytes, next_cursor);
                cursor = next_cursor + dots;
                let duration = note_length_to_seconds(length, state.tempo, dots as u32);
                events.push(PlayEvent::Rest {
                    note_index,
                    duration_seconds: duration,
                });
                total_duration += duration;
                note_index += 1;
            }
            'N' => {
                let (note_number, next_cursor) =
                    parse_required_number(bytes, cursor, token, token_index)?;
                cursor = next_cursor;

                let duration = note_length_to_seconds(state.default_length, state.tempo, 0);
                if note_number == 0 {
                    events.push(PlayEvent::Rest {
                        note_index,
                        duration_seconds: duration,
                    });
                } else {
                    let ideal_frequency_hz =
                        note_number_to_frequency(note_number as i32, token_index)?;
                    events.push(make_note_event(
                        note_index,
                        ideal_frequency_hz,
                        duration,
                        state.note_mode,
                    ));
                }
                total_duration += duration;
                note_index += 1;
            }
            'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' => {
                let mut semitone = match token.to_ascii_uppercase() {
                    'C' => 0,
                    'D' => 2,
                    'E' => 4,
                    'F' => 5,
                    'G' => 7,
                    'A' => 9,
                    'B' => 11,
                    _ => unreachable!(),
                };

                if cursor < bytes.len() {
                    let modifier = bytes[cursor] as char;
                    match modifier {
                        '#' | '+' => {
                            semitone += 1;
                            cursor += 1;
                        }
                        '-' => {
                            semitone -= 1;
                            cursor += 1;
                        }
                        _ => {}
                    }
                }

                let (length, next_cursor) = parse_length_or_default(
                    bytes,
                    cursor,
                    state.default_length,
                    token,
                    token_index,
                )?;
                let dots = count_dots(bytes, next_cursor);
                cursor = next_cursor + dots;

                let duration = note_length_to_seconds(length, state.tempo, dots as u32);
                events.push(make_note_event(
                    note_index,
                    note_frequency_hz(state.octave, semitone),
                    duration,
                    state.note_mode,
                ));
                total_duration += duration;
                note_index += 1;
            }
            _ => {
                return Err(PlayTraceError::UnsupportedToken {
                    token,
                    position: token_index,
                });
            }
        }
    }

    Ok(PlaySequenceTrace {
        name: name.to_string(),
        sequence: sequence.to_string(),
        events,
        total_duration_seconds: total_duration,
    })
}

fn make_note_event(
    note_index: usize,
    ideal_frequency_hz: f32,
    duration_seconds: f32,
    mode: NoteMode,
) -> PlayEvent {
    let pit_divisor = (PC_SPEAKER_CLOCK_HZ / ideal_frequency_hz)
        .round()
        .clamp(1.0, 65_535.0) as u32;
    let quantized_frequency_hz = PC_SPEAKER_CLOCK_HZ / pit_divisor as f32;
    let note_on_seconds = duration_seconds * mode.note_on_ratio();
    let note_off_seconds = (duration_seconds - note_on_seconds).max(0.0);

    PlayEvent::Note {
        note_index,
        ideal_frequency_hz,
        quantized_frequency_hz,
        pit_divisor,
        duration_seconds,
        note_on_seconds,
        note_off_seconds,
        mode,
    }
}

fn parse_number(bytes: &[u8], start: usize) -> Option<(u32, usize)> {
    let mut cursor = start;
    let mut value = 0u32;
    let mut found = false;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if !ch.is_ascii_digit() {
            break;
        }

        found = true;
        value = value * 10 + (ch as u32 - '0' as u32);
        cursor += 1;
    }

    if found {
        Some((value, cursor))
    } else {
        None
    }
}

fn parse_required_number(
    bytes: &[u8],
    start: usize,
    token: char,
    token_position: usize,
) -> Result<(u32, usize), PlayTraceError> {
    parse_number(bytes, start).ok_or(PlayTraceError::MissingNumber {
        token,
        position: token_position,
    })
}

fn parse_length_or_default(
    bytes: &[u8],
    start: usize,
    default_length: u32,
    token: char,
    token_position: usize,
) -> Result<(u32, usize), PlayTraceError> {
    if let Some((number, cursor)) = parse_number(bytes, start) {
        if number == 0 {
            return Err(PlayTraceError::InvalidLength {
                token,
                value: number,
                position: token_position,
            });
        }

        Ok((number, cursor))
    } else {
        Ok((default_length, start))
    }
}

fn count_dots(bytes: &[u8], start: usize) -> usize {
    let mut dots = 0usize;
    let mut cursor = start;

    while cursor < bytes.len() && (bytes[cursor] as char) == '.' {
        dots += 1;
        cursor += 1;
    }

    dots
}

fn note_length_to_seconds(length: u32, tempo: f32, dots: u32) -> f32 {
    let base = (60.0 / tempo) * 4.0 / length as f32;
    let mut duration = base;
    let mut addition = base / 2.0;

    for _ in 0..dots {
        duration += addition;
        addition /= 2.0;
    }

    duration
}

fn note_frequency_hz(octave: i32, semitone: i32) -> f32 {
    let midi = 12 * (octave + PLAY_OCTAVE_MIDI_OFFSET) + semitone;
    440.0 * (2.0f32).powf((midi as f32 - 69.0) / 12.0)
}

fn note_number_to_frequency(note_number: i32, position: usize) -> Result<f32, PlayTraceError> {
    if !(MIN_NOTE_NUMBER..=MAX_NOTE_NUMBER).contains(&note_number) {
        return Err(PlayTraceError::NoteNumberOutOfRange {
            value: note_number,
            position,
        });
    }

    let index = note_number - 1;
    let octave = index / 12;
    let semitone = index % 12;
    Ok(note_frequency_hz(octave, semitone))
}

// PCM synthesis and WAV serialization
pub fn sound_from_play_sequence(sequence: &str, volume: f32) -> Result<Vec<u8>, PlayTraceError> {
    let sample_rate: u32 = 44_100;
    let mut pcm = Vec::<i16>::new();

    for event in trace_sequence("runtime", sequence)?.events {
        match event {
            PlayEvent::Note {
                quantized_frequency_hz,
                duration_seconds,
                note_on_seconds,
                ..
            } => {
                append_pc_speaker_tone(
                    &mut pcm,
                    quantized_frequency_hz,
                    note_on_seconds,
                    volume,
                    sample_rate,
                );
                append_silence(
                    &mut pcm,
                    (duration_seconds - note_on_seconds).max(0.0),
                    sample_rate,
                );
            }
            PlayEvent::Rest {
                duration_seconds, ..
            } => append_silence(&mut pcm, duration_seconds, sample_rate),
        }
    }

    if pcm.is_empty() {
        pcm.push(0);
    }

    apply_pc_speaker_filter(&mut pcm, sample_rate);

    Ok(pcm_to_wav(&pcm, sample_rate))
}

fn apply_pc_speaker_filter(pcm: &mut [i16], sample_rate: u32) {
    if pcm.is_empty() {
        return;
    }

    let sample_rate_hz = sample_rate as f32;

    let highpass_rc = 1.0 / (2.0 * std::f32::consts::PI * PC_SPEAKER_HIGHPASS_HZ);
    let highpass_dt = 1.0 / sample_rate_hz;
    let highpass_alpha = highpass_rc / (highpass_rc + highpass_dt);

    let lowpass_rc = 1.0 / (2.0 * std::f32::consts::PI * PC_SPEAKER_LOWPASS_HZ);
    let lowpass_dt = 1.0 / sample_rate_hz;
    let lowpass_alpha = lowpass_dt / (lowpass_rc + lowpass_dt);

    let mut previous_input = pcm[0] as f32 / i16::MAX as f32;
    let mut highpass_output = 0.0f32;
    let mut lowpass_output = 0.0f32;

    for sample in pcm.iter_mut() {
        let input = *sample as f32 / i16::MAX as f32;
        highpass_output = highpass_alpha * (highpass_output + input - previous_input);
        previous_input = input;

        lowpass_output += lowpass_alpha * (highpass_output - lowpass_output);
        let filtered = lowpass_output.clamp(-1.0, 1.0);
        *sample = (filtered * i16::MAX as f32) as i16;
    }
}

fn append_pc_speaker_tone(
    pcm: &mut Vec<i16>,
    quantized_frequency_hz: f32,
    duration_seconds: f32,
    volume: f32,
    sample_rate: u32,
) {
    if quantized_frequency_hz <= 0.0 || duration_seconds <= 0.0 {
        return;
    }

    let sample_count = (duration_seconds * sample_rate as f32) as usize;
    if sample_count == 0 {
        return;
    }

    let phase_increment = quantized_frequency_hz / sample_rate as f32;
    let mut phase = 0.0f32;

    let attack_samples = (sample_rate as f32 * PC_SPEAKER_ATTACK_SECONDS) as usize;
    let release_samples = (sample_rate as f32 * PC_SPEAKER_RELEASE_SECONDS) as usize;

    for sample_index in 0..sample_count {
        phase += phase_increment;
        if phase >= 1.0 {
            phase -= 1.0;
        }

        let square = if phase < 0.5 { 1.0 } else { -1.0 };

        let attack = if sample_index < attack_samples {
            sample_index as f32 / attack_samples.max(1) as f32
        } else {
            1.0
        };
        let release = if sample_index + release_samples >= sample_count {
            (sample_count.saturating_sub(sample_index)) as f32 / release_samples.max(1) as f32
        } else {
            1.0
        };
        let envelope = attack.min(release).clamp(0.0, 1.0);

        let output = square * envelope * volume * 0.7;
        pcm.push((output * i16::MAX as f32) as i16);
    }
}

fn append_silence(pcm: &mut Vec<i16>, duration_seconds: f32, sample_rate: u32) {
    let sample_count = (duration_seconds.max(0.0) * sample_rate as f32) as usize;
    for _ in 0..sample_count {
        pcm.push(0);
    }
}

fn pcm_to_wav(pcm: &[i16], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let data_bytes_len = (pcm.len() * 2) as u32;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let riff_chunk_size = 36 + data_bytes_len;

    let mut wav = Vec::<u8>::with_capacity((44 + data_bytes_len) as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes_len.to_le_bytes());

    for sample in pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_parser_rejects_unknown_tokens() {
        let result = trace_sequence("bad", "O3L8CZ");
        assert!(matches!(
            result,
            Err(PlayTraceError::UnsupportedToken { token: 'Z', .. })
        ));
    }

    #[test]
    fn test_play_parser_rejects_out_of_range_note_numbers() {
        let result = trace_sequence("bad", "N999");
        assert!(matches!(
            result,
            Err(PlayTraceError::NoteNumberOutOfRange { value: 999, .. })
        ));
    }

    #[test]
    fn test_play_parser_rejects_zero_tempo() {
        let result = trace_sequence("bad", "T0O3C");
        assert!(matches!(
            result,
            Err(PlayTraceError::InvalidTempo { value: 0, .. })
        ));
    }

    #[test]
    fn test_audio_catalog_is_single_source_of_truth() {
        for name in runtime_audio_sequence_names() {
            let sequence = canonical_play_sequence(name).expect("runtime sequence must exist");
            let trace = trace_sequence(name, sequence).expect("runtime sequence must parse");
            assert!(trace.total_duration_seconds > 0.0);
        }
    }

    #[test]
    fn test_play_octave_mapping_places_middle_c_at_o2() {
        let trace = trace_sequence("middle_c", "O2C").expect("O2C must parse");
        let note = match &trace.events[0] {
            PlayEvent::Note {
                ideal_frequency_hz, ..
            } => *ideal_frequency_hz,
            PlayEvent::Rest { .. } => panic!("O2C must produce a note"),
        };

        assert!((note - 261.625_58).abs() < 0.01);
    }
}
