#!/usr/bin/env python3
"""Compare QBASIC full-scene checkpoint log against Rust full-scene checkpoints."""

from __future__ import annotations

import argparse
from pathlib import Path

from compare_gorillas_render import (
    MAX_DEFAULT_DIFF_COORDS,
    compare,
    load_rust_manifest,
    parse_oracle_log,
)


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
        print("Scene checkpoint mismatch:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("Scene checkpoints match exactly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
