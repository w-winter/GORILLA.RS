#!/usr/bin/env python3
"""Compare PLAY strings extracted from a local QBASIC source file against Rust's canonical catalog"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("basic_catalog", type=Path, help="JSON array from extract_gorillas_play_strings.py")
    parser.add_argument("rust_catalog", type=Path, help="JSON from gorillas_play_trace")
    args = parser.parse_args()

    basic_items = json.loads(args.basic_catalog.read_text())
    rust_items = json.loads(args.rust_catalog.read_text())

    basic_counts = Counter(basic_items)
    rust_counts = Counter(item["sequence"] for item in rust_items)

    errors: list[str] = []
    for sequence in sorted(set(basic_counts) | set(rust_counts)):
        if basic_counts[sequence] != rust_counts[sequence]:
            errors.append(
                f"sequence count mismatch for {sequence!r}: basic={basic_counts[sequence]} rust={rust_counts[sequence]}"
            )

    if errors:
        print("PLAY catalog mismatch:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("PLAY catalog matches by unique sequence counts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
