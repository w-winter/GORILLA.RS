#!/usr/bin/env python3
"""Segment and align a QBASIC audio oracle capture against the Rust PLAY renders."""

from __future__ import annotations

import argparse
import json
import math
import re
import struct
import wave
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_wav", type=Path)
    parser.add_argument("oracle_log", type=Path)
    parser.add_argument("capture_manifest", type=Path)
    parser.add_argument(
        "--rust-wav-dir",
        type=Path,
        default=None,
        help="Directory containing Rust per-sequence WAVs for duration comparison",
    )
    parser.add_argument(
        "--report-path",
        type=Path,
        default=None,
        help="Write JSON analysis report to this path",
    )
    parser.add_argument(
        "--split-dir",
        type=Path,
        default=None,
        help="Write detected per-sequence WAV slices into this directory",
    )
    parser.add_argument(
        "--window-ms",
        type=float,
        default=10.0,
        help="Analysis window size in milliseconds",
    )
    parser.add_argument(
        "--threshold-db",
        type=float,
        default=-55.0,
        help="Windows at or above this dBFS count as active audio",
    )
    parser.add_argument(
        "--min-silence-ms",
        type=float,
        default=120.0,
        help="Minimum silence duration required to separate adjacent segments",
    )
    parser.add_argument(
        "--min-segment-ms",
        type=float,
        default=80.0,
        help="Discard detected segments shorter than this duration",
    )
    parser.add_argument(
        "--padding-ms",
        type=float,
        default=40.0,
        help="Expand each exported segment by this much on both sides",
    )
    return parser.parse_args()


def read_wav_samples(path: Path) -> tuple[dict[str, int], bytes]:
    with wave.open(str(path), "rb") as wav_file:
        params = {
            "channels": wav_file.getnchannels(),
            "sample_width": wav_file.getsampwidth(),
            "sample_rate": wav_file.getframerate(),
            "frame_count": wav_file.getnframes(),
            "compression_type": wav_file.getcomptype(),
            "compression_name": wav_file.getcompname(),
        }
        frames = wav_file.readframes(params["frame_count"])
    return params, frames


def mono_window_dbfs(
    frames: bytes,
    *,
    channels: int,
    sample_width: int,
    sample_rate: int,
    window_ms: float,
) -> list[float]:
    if sample_width != 2:
        raise ValueError(f"unsupported sample width: {sample_width} bytes")

    samples = struct.unpack("<" + "h" * (len(frames) // sample_width), frames)
    max_amplitude = float((1 << (8 * sample_width - 1)) - 1)
    window_frames = max(1, round(sample_rate * window_ms / 1000.0))
    dbfs_values: list[float] = []

    for frame_start in range(0, len(samples) // channels, window_frames):
        mono_samples: list[float] = []
        frame_end = min(len(samples) // channels, frame_start + window_frames)
        for frame_index in range(frame_start, frame_end):
            base = frame_index * channels
            if channels == 1:
                mono_samples.append(float(samples[base]))
            else:
                mono_samples.append(sum(samples[base : base + channels]) / channels)

        if not mono_samples:
            continue

        rms = math.sqrt(sum(sample * sample for sample in mono_samples) / len(mono_samples))
        dbfs_values.append(-120.0 if rms == 0 else 20.0 * math.log10(rms / max_amplitude))

    return dbfs_values


def detect_segments(
    dbfs_values: list[float],
    *,
    threshold_db: float,
    window_ms: float,
    min_silence_ms: float,
    min_segment_ms: float,
) -> list[dict[str, float]]:
    active = [value >= threshold_db for value in dbfs_values]
    min_silence_windows = max(1, round(min_silence_ms / window_ms))
    min_segment_windows = max(1, round(min_segment_ms / window_ms))

    segments: list[dict[str, float]] = []
    start_index: int | None = None
    silent_run = 0

    for index, is_active in enumerate(active):
        if is_active:
            if start_index is None:
                start_index = index
            silent_run = 0
            continue

        if start_index is None:
            continue

        silent_run += 1
        if silent_run >= min_silence_windows:
            end_index = index - silent_run + 1
            if end_index - start_index >= min_segment_windows:
                segments.append(
                    {
                        "start_seconds": start_index * window_ms / 1000.0,
                        "end_seconds": end_index * window_ms / 1000.0,
                        "duration_seconds": (end_index - start_index) * window_ms / 1000.0,
                    }
                )
            start_index = None
            silent_run = 0

    if start_index is not None:
        end_index = len(active)
        if end_index - start_index >= min_segment_windows:
            segments.append(
                {
                    "start_seconds": start_index * window_ms / 1000.0,
                    "end_seconds": end_index * window_ms / 1000.0,
                    "duration_seconds": (end_index - start_index) * window_ms / 1000.0,
                }
            )

    return segments


def load_sequence_names(path: Path) -> list[str]:
    names: list[str] = []
    for line in path.read_text(encoding="ascii").splitlines():
        match = re.match(r"SEQUENCE name=(.+)$", line.strip())
        if match:
            names.append(match.group(1))
    return names


def load_manifest(path: Path) -> list[dict[str, Any]]:
    items = json.loads(path.read_text())
    if not isinstance(items, list):
        raise ValueError("capture manifest must be a JSON array")
    return items


def wav_duration_seconds(path: Path) -> float:
    with wave.open(str(path), "rb") as wav_file:
        return wav_file.getnframes() / wav_file.getframerate()


def partition_segments_by_expected_durations(
    raw_segments: list[dict[str, float]],
    expected_durations: list[float],
) -> list[dict[str, float]] | None:
    if len(raw_segments) < len(expected_durations):
        return None

    penalty_scale = 8.0
    max_internal_gap_without_penalty = 0.25
    raw_count = len(raw_segments)
    expected_count = len(expected_durations)
    costs: list[list[float]] = [[math.inf] * (expected_count + 1) for _ in range(raw_count + 1)]
    backpointers: list[list[int | None]] = [[None] * (expected_count + 1) for _ in range(raw_count + 1)]
    costs[0][0] = 0.0

    for raw_index in range(1, raw_count + 1):
        for expected_index in range(1, min(raw_index, expected_count) + 1):
            best_cost = math.inf
            best_start = None
            for start in range(expected_index - 1, raw_index):
                previous_cost = costs[start][expected_index - 1]
                if math.isinf(previous_cost):
                    continue

                start_seconds = raw_segments[start]["start_seconds"]
                end_seconds = raw_segments[raw_index - 1]["end_seconds"]
                span_duration = end_seconds - start_seconds
                max_internal_gap = 0.0
                for gap_index in range(start, raw_index - 1):
                    gap = raw_segments[gap_index + 1]["start_seconds"] - raw_segments[gap_index]["end_seconds"]
                    if gap > max_internal_gap:
                        max_internal_gap = gap

                penalty = max(0.0, max_internal_gap - max_internal_gap_without_penalty) * penalty_scale
                cost = previous_cost + abs(span_duration - expected_durations[expected_index - 1]) + penalty
                if cost < best_cost:
                    best_cost = cost
                    best_start = start

            costs[raw_index][expected_index] = best_cost
            backpointers[raw_index][expected_index] = best_start

    if math.isinf(costs[raw_count][expected_count]):
        return None

    grouped_segments: list[dict[str, float]] = []
    raw_index = raw_count
    expected_index = expected_count
    while expected_index > 0:
        start = backpointers[raw_index][expected_index]
        if start is None:
            return None

        start_seconds = raw_segments[start]["start_seconds"]
        end_seconds = raw_segments[raw_index - 1]["end_seconds"]
        grouped_segments.append(
            {
                "start_seconds": start_seconds,
                "end_seconds": end_seconds,
                "duration_seconds": end_seconds - start_seconds,
                "raw_segment_start_index": start,
                "raw_segment_end_index": raw_index - 1,
            }
        )
        raw_index = start
        expected_index -= 1

    grouped_segments.reverse()
    return grouped_segments


def build_report(
    *,
    capture_wav: Path,
    params: dict[str, int],
    sequence_names: list[str],
    manifest_items: list[dict[str, Any]],
    raw_segments: list[dict[str, float]],
    aligned_segments: list[dict[str, float]],
    rust_wav_dir: Path | None,
    threshold_db: float,
    window_ms: float,
    min_silence_ms: float,
    min_segment_ms: float,
    padding_ms: float,
) -> dict[str, Any]:
    manifest_by_name = {item["name"]: item for item in manifest_items}
    capture_duration = params["frame_count"] / params["sample_rate"]

    report_segments: list[dict[str, Any]] = []
    for index, name in enumerate(sequence_names):
        manifest_item = manifest_by_name.get(name)
        rust_duration = None
        if rust_wav_dir is not None:
            rust_path = rust_wav_dir / f"{name}.wav"
            if rust_path.exists():
                rust_duration = wav_duration_seconds(rust_path)

        segment_data = aligned_segments[index] if index < len(aligned_segments) else None
        entry: dict[str, Any] = {
            "index": index,
            "name": name,
            "sequence": None if manifest_item is None else manifest_item.get("sequence"),
            "capture_sequence": None if manifest_item is None else manifest_item.get("capture_sequence"),
            "rust_duration_seconds": rust_duration,
            "detected": segment_data is not None,
        }
        if segment_data is not None:
            entry.update(segment_data)
            if rust_duration is not None:
                entry["duration_delta_seconds"] = segment_data["duration_seconds"] - rust_duration
        report_segments.append(entry)

    return {
        "capture": {
            "path": str(capture_wav),
            "duration_seconds": capture_duration,
            "sample_rate": params["sample_rate"],
            "channels": params["channels"],
            "sample_width": params["sample_width"],
        },
        "analysis": {
            "threshold_db": threshold_db,
            "window_ms": window_ms,
            "min_silence_ms": min_silence_ms,
            "min_segment_ms": min_segment_ms,
            "padding_ms": padding_ms,
            "raw_segment_count": len(raw_segments),
            "aligned_segment_count": len(aligned_segments),
            "expected_sequence_count": len(sequence_names),
            "leading_silence_seconds": aligned_segments[0]["start_seconds"] if aligned_segments else capture_duration,
            "trailing_silence_seconds": (
                capture_duration - aligned_segments[-1]["end_seconds"] if aligned_segments else capture_duration
            ),
        },
        "segments": report_segments,
    }


def write_wav_slice(
    *,
    out_path: Path,
    source_frames: bytes,
    params: dict[str, int],
    start_seconds: float,
    end_seconds: float,
) -> None:
    bytes_per_frame = params["channels"] * params["sample_width"]
    start_frame = max(0, min(params["frame_count"], round(start_seconds * params["sample_rate"])))
    end_frame = max(start_frame, min(params["frame_count"], round(end_seconds * params["sample_rate"])))
    frame_slice = source_frames[start_frame * bytes_per_frame : end_frame * bytes_per_frame]

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(out_path), "wb") as wav_file:
        wav_file.setnchannels(params["channels"])
        wav_file.setsampwidth(params["sample_width"])
        wav_file.setframerate(params["sample_rate"])
        wav_file.writeframes(frame_slice)


def export_splits(
    *,
    split_dir: Path,
    source_frames: bytes,
    params: dict[str, int],
    report: dict[str, Any],
    padding_ms: float,
) -> None:
    padding_seconds = padding_ms / 1000.0
    for segment in report["segments"]:
        if not segment["detected"]:
            continue

        export_start_seconds = segment["start_seconds"]
        export_end_seconds = segment["end_seconds"]
        rust_duration = segment.get("rust_duration_seconds")
        if rust_duration is not None:
            export_end_seconds = max(export_end_seconds, export_start_seconds + rust_duration)

        start_seconds = max(0.0, export_start_seconds - padding_seconds)
        end_seconds = min(report["capture"]["duration_seconds"], export_end_seconds + padding_seconds)
        write_wav_slice(
            out_path=split_dir / f"{segment['index']:02d}_{segment['name']}.wav",
            source_frames=source_frames,
            params=params,
            start_seconds=start_seconds,
            end_seconds=end_seconds,
        )


def print_summary(report: dict[str, Any]) -> None:
    analysis = report["analysis"]
    print(
        "Detected "
        f"{analysis['raw_segment_count']} raw segments and aligned "
        f"{analysis['aligned_segment_count']} segments for "
        f"{analysis['expected_sequence_count']} expected sequences"
    )
    print(
        f"Leading silence: {analysis['leading_silence_seconds']:.3f}s | "
        f"Trailing silence: {analysis['trailing_silence_seconds']:.3f}s"
    )
    for segment in report["segments"]:
        if not segment["detected"]:
            print(f"- {segment['index']:02d} {segment['name']}: not detected")
            continue
        delta = segment.get("duration_delta_seconds")
        if delta is None:
            delta_suffix = ""
        else:
            delta_suffix = f" | delta_vs_rust={delta:+.3f}s"
        print(
            f"- {segment['index']:02d} {segment['name']}: "
            f"{segment['start_seconds']:.3f}-{segment['end_seconds']:.3f}s "
            f"dur={segment['duration_seconds']:.3f}s"
            f"{delta_suffix}"
        )


def main() -> int:
    args = parse_args()
    params, source_frames = read_wav_samples(args.capture_wav)
    dbfs_values = mono_window_dbfs(
        source_frames,
        channels=params["channels"],
        sample_width=params["sample_width"],
        sample_rate=params["sample_rate"],
        window_ms=args.window_ms,
    )
    raw_segments = detect_segments(
        dbfs_values,
        threshold_db=args.threshold_db,
        window_ms=args.window_ms,
        min_silence_ms=args.min_silence_ms,
        min_segment_ms=args.min_segment_ms,
    )
    sequence_names = load_sequence_names(args.oracle_log)
    manifest_items = load_manifest(args.capture_manifest)

    expected_durations: list[float] = []
    if args.rust_wav_dir is not None:
        for name in sequence_names:
            rust_path = args.rust_wav_dir / f"{name}.wav"
            if not rust_path.exists():
                expected_durations = []
                break
            expected_durations.append(wav_duration_seconds(rust_path))

    aligned_segments = raw_segments
    if expected_durations and len(raw_segments) != len(sequence_names):
        partitioned = partition_segments_by_expected_durations(raw_segments, expected_durations)
        if partitioned is not None:
            aligned_segments = partitioned

    report = build_report(
        capture_wav=args.capture_wav,
        params=params,
        sequence_names=sequence_names,
        manifest_items=manifest_items,
        raw_segments=raw_segments,
        aligned_segments=aligned_segments,
        rust_wav_dir=args.rust_wav_dir,
        threshold_db=args.threshold_db,
        window_ms=args.window_ms,
        min_silence_ms=args.min_silence_ms,
        min_segment_ms=args.min_segment_ms,
        padding_ms=args.padding_ms,
    )

    print_summary(report)

    if args.report_path is not None:
        args.report_path.parent.mkdir(parents=True, exist_ok=True)
        args.report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    if args.split_dir is not None:
        export_splits(
            split_dir=args.split_dir,
            source_frames=source_frames,
            params=params,
            report=report,
            padding_ms=args.padding_ms,
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
