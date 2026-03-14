#!/usr/bin/env python3
"""Compare a BASIC oracle Gorillas trace against the Rust trace exporter."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


def parse_key_values(parts: list[str]) -> dict[str, str]:
    values: dict[str, str] = {}
    for part in parts:
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        values[key] = value
    return values


def parse_oracle_log(path: Path) -> dict[str, Any]:
    gravity = None
    wind = None
    buildings = []
    gorillas = [None, None]
    shots: dict[int, dict[str, Any]] = {}

    for raw_line in path.read_text(encoding="ascii").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("'"):
            continue

        parts = line.split()
        record = parts[0]
        values = parse_key_values(parts[1:])

        if record == "ROUND":
            gravity = float(values["gravity"])
            wind = int(values["wind"])
        elif record == "BUILDING":
            buildings.append(
                {
                    "x": float(values["x"]),
                    "width": float(values["width"]),
                    "top_y": float(values["top_y"]),
                }
            )
        elif record == "GORILLA":
            player = int(values["player"]) - 1
            gorillas[player] = {"x": float(values["x"]), "y": float(values["y"])}
        elif record == "SHOT":
            shot_index = int(values["shot"])
            shots[shot_index] = {
                "player": int(values["player"]),
                "input_angle": float(values["angle"]),
                "effective_angle": float(values["effective_angle"]),
                "velocity": float(values["velocity"]),
                "start_x": float(values["start_x"]),
                "start_y": float(values["start_y"]),
                "points": [],
            }
        elif record == "POINT":
            shot_index = int(values["shot"])
            shots[shot_index]["points"].append(
                {
                    "step": int(values["step"]),
                    "t": float(values["t"]),
                    "x": float(values["x"]),
                    "y": float(values["y"]),
                    "rotation": int(values["rotation"]),
                    "in_sun": values["in_sun"] == "1",
                }
            )
        elif record == "OUTCOME":
            shot_index = int(values["shot"])
            outcome: dict[str, Any] = {"kind": values["kind"]}
            if "x" in values:
                outcome["x"] = float(values["x"])
            if "y" in values:
                outcome["y"] = float(values["y"])
            if "gorilla_index" in values:
                outcome["gorilla_index"] = int(values["gorilla_index"])
            if "building_index" in values:
                outcome["building_index"] = int(values["building_index"])
            shots[shot_index]["outcome"] = outcome

    ordered_shots = [shots[index] for index in sorted(shots)]

    return {
        "gravity": gravity,
        "round": {
            "wind": wind,
            "buildings": buildings,
            "gorillas": gorillas,
        },
        "shots": ordered_shots,
    }


def load_trace(path: Path) -> dict[str, Any]:
    if path.suffix.lower() == ".json":
        return json.loads(path.read_text())
    return parse_oracle_log(path)


def approx_equal(left: float, right: float, tolerance: float) -> bool:
    return math.isclose(left, right, rel_tol=tolerance, abs_tol=tolerance)


def compare(left: dict[str, Any], right: dict[str, Any], tolerance: float) -> list[str]:
    errors: list[str] = []

    if not approx_equal(left["gravity"], right["gravity"], tolerance):
        errors.append(f"gravity mismatch: {left['gravity']} vs {right['gravity']}")

    if left["round"]["wind"] != right["round"]["wind"]:
        errors.append(f"wind mismatch: {left['round']['wind']} vs {right['round']['wind']}")

    if len(left["round"]["buildings"]) != len(right["round"]["buildings"]):
        errors.append(
            f"building count mismatch: {len(left['round']['buildings'])} vs {len(right['round']['buildings'])}"
        )
    else:
        for index, (lhs, rhs) in enumerate(zip(left["round"]["buildings"], right["round"]["buildings"], strict=True)):
            for field in ("x", "width", "top_y"):
                if not approx_equal(lhs[field], rhs[field], tolerance):
                    errors.append(
                        f"building {index} field {field} mismatch: {lhs[field]} vs {rhs[field]}"
                    )

    for index, (lhs, rhs) in enumerate(zip(left["round"]["gorillas"], right["round"]["gorillas"], strict=True)):
        for field in ("x", "y"):
            if not approx_equal(lhs[field], rhs[field], tolerance):
                errors.append(
                    f"gorilla {index} field {field} mismatch: {lhs[field]} vs {rhs[field]}"
                )

    if len(left["shots"]) != len(right["shots"]):
        errors.append(f"shot count mismatch: {len(left['shots'])} vs {len(right['shots'])}")
        return errors

    for shot_index, (lhs_shot, rhs_shot) in enumerate(zip(left["shots"], right["shots"], strict=True), start=1):
        for field in ("player", "input_angle", "effective_angle", "velocity", "start_x", "start_y"):
            if field == "player":
                if lhs_shot[field] != rhs_shot[field]:
                    errors.append(f"shot {shot_index} player mismatch: {lhs_shot[field]} vs {rhs_shot[field]}")
            elif not approx_equal(lhs_shot[field], rhs_shot[field], tolerance):
                errors.append(
                    f"shot {shot_index} field {field} mismatch: {lhs_shot[field]} vs {rhs_shot[field]}"
                )

        lhs_outcome = lhs_shot.get("outcome", {})
        rhs_outcome = rhs_shot.get("outcome", {})
        if lhs_outcome.get("kind") != rhs_outcome.get("kind"):
            errors.append(
                f"shot {shot_index} outcome mismatch: {lhs_outcome.get('kind')} vs {rhs_outcome.get('kind')}"
            )

        for field in ("x", "y"):
            if field in lhs_outcome or field in rhs_outcome:
                if not approx_equal(lhs_outcome.get(field, 0.0), rhs_outcome.get(field, 0.0), tolerance):
                    errors.append(
                        f"shot {shot_index} outcome {field} mismatch: {lhs_outcome.get(field)} vs {rhs_outcome.get(field)}"
                    )

        for field in ("building_index", "gorilla_index"):
            if lhs_outcome.get(field) != rhs_outcome.get(field):
                if field in lhs_outcome or field in rhs_outcome:
                    errors.append(
                        f"shot {shot_index} outcome {field} mismatch: {lhs_outcome.get(field)} vs {rhs_outcome.get(field)}"
                    )

        lhs_points = lhs_shot["points"]
        rhs_points = rhs_shot["points"]
        if len(lhs_points) != len(rhs_points):
            errors.append(
                f"shot {shot_index} point count mismatch: {len(lhs_points)} vs {len(rhs_points)}"
            )
            continue

        for point_index, (lhs_point, rhs_point) in enumerate(zip(lhs_points, rhs_points, strict=True)):
            if lhs_point["step"] != rhs_point["step"]:
                errors.append(
                    f"shot {shot_index} point {point_index} step mismatch: {lhs_point['step']} vs {rhs_point['step']}"
                )
            if lhs_point["rotation"] != rhs_point["rotation"]:
                errors.append(
                    f"shot {shot_index} point {point_index} rotation mismatch: {lhs_point['rotation']} vs {rhs_point['rotation']}"
                )
            if lhs_point["in_sun"] != rhs_point["in_sun"]:
                errors.append(
                    f"shot {shot_index} point {point_index} in_sun mismatch: {lhs_point['in_sun']} vs {rhs_point['in_sun']}"
                )
            for field in ("t", "x", "y"):
                if not approx_equal(lhs_point[field], rhs_point[field], tolerance):
                    errors.append(
                        f"shot {shot_index} point {point_index} field {field} mismatch: {lhs_point[field]} vs {rhs_point[field]}"
                    )
                    break

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=Path, help="Oracle BASIC log or JSON trace")
    parser.add_argument("right", type=Path, help="Rust JSON trace")
    parser.add_argument("--tolerance", type=float, default=0.001)
    args = parser.parse_args()

    left = load_trace(args.left)
    right = load_trace(args.right)
    errors = compare(left, right, args.tolerance)

    if errors:
        print("Trace mismatch:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("Trace match within tolerance")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
