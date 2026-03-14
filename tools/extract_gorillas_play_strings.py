#!/usr/bin/env python3
"""Extract QBASIC PLAY strings from a local GORILLA.BAS source file"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

PLAY_RE = re.compile(r'PLAY\s+"([^"]+)"', re.IGNORECASE)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "source",
        type=Path,
        help="Path to a lawful local copy of GORILLA.BAS or a compatible oracle source file",
    )
    args = parser.parse_args()

    text = args.source.read_text(encoding="ascii", errors="ignore")
    matches = PLAY_RE.findall(text)
    print(json.dumps(matches, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
