#!/usr/bin/env python3
"""Compare QBASIC render checkpoint log against Rust render checkpoints.

By default this tool only prints a concise mismatch summary.

Use --dump-diff-dir to emit per-checkpoint diff artifacts (PPM + JSON coordinate list) to help diagnose
pixel-level mismatches without wading through huge logs.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

MAX_DEFAULT_DIFF_COORDS = 200


def _index_to_gray(index_value: int) -> int:
    # Most checkpoints are 0..15 palette indices. Clamping keeps visualization stable even if a bug
    # produces out-of-range indices
    clamped = 0 if index_value < 0 else 15 if index_value > 15 else index_value
    return int(clamped * 255 / 15)


def _pgm_bytes(width: int, height: int, pixels: list[int]) -> bytes:
    header = f"P5\n{width} {height}\n255\n".encode("ascii")
    payload = bytes(_index_to_gray(p) for p in pixels)
    return header + payload


def _diff_ppm_bytes(width: int, height: int, lhs: list[int], rhs: list[int]) -> bytes:
    header = f"P6\n{width} {height}\n255\n".encode("ascii")
    payload = bytearray()

    for lp, rp in zip(lhs, rhs, strict=True):
        if lp == rp:
            gray = _index_to_gray(lp)
            payload.extend((gray, gray, gray))
        else:
            # Highlight mismatches in red
            payload.extend((255, 0, 0))

    return header + bytes(payload)


def _write_diff_artifacts(
    *,
    out_dir: Path,
    checkpoint_name: str,
    lhs: dict[str, Any],
    rhs: dict[str, Any],
    mismatch_count: int,
    mismatch_coords: list[dict[str, int]],
) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)

    width = int(lhs["width"])
    height = int(lhs["height"])

    (out_dir / f"{checkpoint_name}.oracle.pgm").write_bytes(_pgm_bytes(width, height, lhs["pixels"]))
    (out_dir / f"{checkpoint_name}.rust.pgm").write_bytes(_pgm_bytes(width, height, rhs["pixels"]))
    (out_dir / f"{checkpoint_name}.diff.ppm").write_bytes(
        _diff_ppm_bytes(width, height, lhs["pixels"], rhs["pixels"])
    )

    (out_dir / f"{checkpoint_name}.diff.json").write_text(
        json.dumps(
            {
                "name": checkpoint_name,
                "width": width,
                "height": height,
                "mismatch_count": mismatch_count,
                "diffs": mismatch_coords,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def parse_key_values(parts: list[str]) -> dict[str, str]:
    values: dict[str, str] = {}
    for part in parts:
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        values[key] = value
    return values


def parse_oracle_log(path: Path) -> dict[str, dict[str, Any]]:
    checkpoints: dict[str, dict[str, Any]] = {}

    for raw_line in path.read_text(encoding="ascii").splitlines():
        line = raw_line.strip()
        if not line:
            continue

        parts = line.split()
        record = parts[0]
        values = parse_key_values(parts[1:])

        if record == "CHECKPOINT":
            checkpoints[values["name"]] = {
                "name": values["name"],
                "width": int(values["width"]),
                "height": int(values["height"]),
                "pixels": [],
            }
        elif record == "ROW":
            row = [int(item) for item in values["pixels"].split(",") if item != ""]
            checkpoints[values["name"]]["pixels"].extend(row)

    return checkpoints


def load_rust_manifest(path: Path) -> dict[str, dict[str, Any]]:
    items = json.loads(path.read_text())
    return {item["name"]: item for item in items}


def compare(
    left: dict[str, dict[str, Any]],
    right: dict[str, dict[str, Any]],
    *,
    dump_diff_dir: Path | None = None,
    max_diff_coords: int = MAX_DEFAULT_DIFF_COORDS,
) -> list[str]:
    errors: list[str] = []
    names = sorted(set(left) | set(right))

    for name in names:
        if name not in left:
            errors.append(f"missing oracle checkpoint: {name}")
            continue
        if name not in right:
            errors.append(f"missing rust checkpoint: {name}")
            continue

        lhs = left[name]
        rhs = right[name]

        if lhs["width"] != rhs["width"] or lhs["height"] != rhs["height"]:
            errors.append(
                f"{name}: size mismatch {lhs['width']}x{lhs['height']} vs {rhs['width']}x{rhs['height']}"
            )
            continue

        if len(lhs["pixels"]) != len(rhs["pixels"]):
            errors.append(
                f"{name}: pixel count mismatch {len(lhs['pixels'])} vs {len(rhs['pixels'])}"
            )
            continue

        width = int(lhs["width"])
        mismatch_count = 0
        mismatch_coords: list[dict[str, int]] = []
        first_diff: tuple[int, int, int, int] | None = None

        for index, (lp, rp) in enumerate(zip(lhs["pixels"], rhs["pixels"], strict=True)):
            if lp == rp:
                continue

            mismatch_count += 1
            y, x = divmod(index, width)

            if first_diff is None:
                first_diff = (x, y, int(lp), int(rp))

            if dump_diff_dir is not None and len(mismatch_coords) < max_diff_coords:
                mismatch_coords.append({"x": x, "y": y, "oracle": int(lp), "rust": int(rp)})

        if mismatch_count:
            if first_diff is None:
                errors.append(f"{name}: {mismatch_count} mismatched pixels")
            else:
                x, y, lp, rp = first_diff
                errors.append(
                    f"{name}: {mismatch_count} mismatched pixels; first at ({x},{y}) oracle={lp} rust={rp}"
                )

            if dump_diff_dir is not None:
                _write_diff_artifacts(
                    out_dir=dump_diff_dir,
                    checkpoint_name=name,
                    lhs=lhs,
                    rhs=rhs,
                    mismatch_count=mismatch_count,
                    mismatch_coords=mismatch_coords,
                )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle_log", type=Path)
    parser.add_argument("rust_manifest", type=Path)
    parser.add_argument(
        "--dump-diff-dir",
        type=Path,
        default=None,
        help="Write diff artifacts (<name>.diff.ppm + .pgm + .json) for mismatching checkpoints",
    )
    parser.add_argument(
        "--max-diff-coords",
        type=int,
        default=MAX_DEFAULT_DIFF_COORDS,
        help="Max mismatching pixel coordinates to include per checkpoint in <name>.diff.json",
    )
    args = parser.parse_args()

    oracle = parse_oracle_log(args.oracle_log)
    rust = load_rust_manifest(args.rust_manifest)
    errors = compare(
        oracle,
        rust,
        dump_diff_dir=args.dump_diff_dir,
        max_diff_coords=args.max_diff_coords,
    )

    if errors:
        print("Render checkpoint mismatch:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("Render checkpoints match exactly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
