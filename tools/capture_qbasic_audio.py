#!/usr/bin/env python3
"""Generate a QB45 audio harness and export Rust-side WAVs for manual emulator capture."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parents[1]
PLAY_REF_PATH = REPO_ROOT / 'src' / 'play_ref.rs'
DEFAULT_DOS_ROOT = Path.home() / 'dos'
DEFAULT_WORKSPACE_NAME = 'gorillas'
DEFAULT_HARNESS_NAME = 'GORAUDIO.BAS'
DEFAULT_LOG_NAME = 'GORAUDIO.LOG'
DEFAULT_CAPTURE_ROOT = REPO_ROOT / 'local' / 'artifacts' / 'qbasic_audio'
DEFAULT_CAPTURE_FILENAME = 'qbasic_audio_capture.wav'
DEFAULT_MANUAL_STEPS_NAME = 'manual_capture_steps.txt'
INTER_SEQUENCE_GAP_SECONDS = 0.5


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument('--dos-root', type=Path, default=DEFAULT_DOS_ROOT)
    parser.add_argument('--workspace-name', default=DEFAULT_WORKSPACE_NAME)
    parser.add_argument('--qb-dir', default='QB45')
    parser.add_argument('--dosbox', default='dosbox-x')
    parser.add_argument('--capture-dir', type=Path)
    parser.add_argument('--harness-name', default=DEFAULT_HARNESS_NAME)
    parser.add_argument('--log-name', default=DEFAULT_LOG_NAME)
    parser.add_argument('--no-rust-wavs', action='store_true')
    parser.add_argument('--keep-capture-raw-name', action='store_true')
    parser.add_argument('--time-limit', type=int, default=120)
    parser.add_argument('--generate-only', action='store_true')
    return parser.parse_args()


def load_play_catalog() -> tuple[dict[str, str], list[str]]:
    source = PLAY_REF_PATH.read_text()
    named_sequences = dict(
        re.findall(r'name:\s*"([^"]+)"\s*,\s*sequence:\s*"([^"]+)"', source, re.S)
    )
    runtime_block = re.search(
        r'const RUNTIME_AUDIO_SEQUENCE_NAMES:\s*&\[&str\]\s*=\s*&\[(.*?)\];',
        source,
        re.S,
    )
    if runtime_block is None:
        raise RuntimeError('could not find RUNTIME_AUDIO_SEQUENCE_NAMES in src/play_ref.rs')
    runtime_names = re.findall(r'"([^"]+)"', runtime_block.group(1))
    if not named_sequences or not runtime_names:
        raise RuntimeError('failed to parse PLAY catalog from src/play_ref.rs')
    return named_sequences, runtime_names


def force_foreground(sequence: str) -> str:
    if len(sequence) >= 2 and sequence[0].upper() == 'M' and sequence[1].upper() in {'B', 'F'}:
        return f'MF{sequence[2:]}'
    return f'MF{sequence}'


def dos_path(parts: Iterable[str]) -> str:
    return '\\'.join(parts)


def build_basic_harness(
    sequence_order: list[str],
    sequence_map: dict[str, str],
    workspace_name: str,
    log_name: str,
) -> str:
    lines = [
        'DEFINT A-Z',
        '',
        'DECLARE SUB PauseSeconds (t!)',
        '',
        'SCREEN 0',
        'WIDTH 80',
        'COLOR 7, 0',
        'CLS',
        f'OPEN "C:\\{workspace_name.upper()}\\{log_name}" FOR OUTPUT AS #1',
        'PRINT #1, "AUDIO_ORACLE"',
        'PRINT "GORILLAS AUDIO ORACLE"',
        '',
    ]

    for name in sequence_order:
        sequence = force_foreground(sequence_map[name])
        lines.extend(
            [
                f'PRINT #1, "SEQUENCE name={name}"',
                f'PRINT "{name}"',
                f'PLAY "{sequence}"',
                f'CALL PauseSeconds({INTER_SEQUENCE_GAP_SECONDS})',
                '',
            ]
        )

    lines.extend(
        [
            'CLOSE #1',
            'SYSTEM',
            '',
            'SUB PauseSeconds (t!)',
            '  s! = TIMER',
            '  DO',
            '  LOOP UNTIL TIMER - s! >= t!',
            'END SUB',
            '',
        ]
    )
    return '\n'.join(lines)


def build_dosbox_conf(
    dos_root: Path,
    workspace_name: str,
    qb_dir: str,
    harness_name: str,
    capture_dir: Path,
) -> str:
    escaped_dos_root = str(dos_root).replace('"', '""')
    escaped_capture_dir = str(capture_dir).replace('"', '""')
    workspace_harness = dos_path([workspace_name.upper(), harness_name])
    return f"""[dosbox]
captures = {escaped_capture_dir}
show recorded filename = false

[autoexec]
@echo off
mount c \"{escaped_dos_root}\"
c:
cd \\{qb_dir}
dx-capture wav qb.exe /run c:\\{workspace_harness}
exit
"""


def export_rust_wavs(capture_dir: Path) -> None:
    rust_wav_dir = capture_dir / 'rust_wavs'
    rust_wav_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ['cargo', 'run', '--quiet', '--bin', 'gorillas_play_wav', '--', str(rust_wav_dir)],
        cwd=REPO_ROOT,
        check=True,
    )


def build_manual_capture_steps(
    dos_root: Path,
    qb_dir_name: str,
    workspace_name: str,
    harness_name: str,
    log_name: str,
    capture_dir: Path,
) -> str:
    workspace_harness = dos_path([workspace_name.upper(), harness_name])
    capture_target = capture_dir / DEFAULT_CAPTURE_FILENAME
    return '\n'.join(
        [
            'Manual QBASIC audio capture workflow',
            '',
            f'Host DOS root: {dos_root}',
            f'Capture artifact directory: {capture_dir}',
            f'Expected final capture path: {capture_target}',
            '',
            'In DOSBox Staging or another working DOS emulator session:',
            f'  mount c {dos_root}',
            '  c:',
            f'  cd \\{qb_dir_name}',
            f'  qb /run c:\\{workspace_harness}',
            '',
            'After the harness finishes:',
            f'  1. Save or copy the emulator audio capture to {capture_target}',
            f'  2. Keep the DOS-side log next to it as {capture_dir / log_name}',
            '  3. Compare against the Rust renders in the rust_wavs/ subdirectory',
            '',
            'The generated harness prints each sequence name before playback and also writes',
            'the same sequence order to GORAUDIO.LOG for later slicing and comparison',
            '',
        ]
    )


def newest_wav(capture_dir: Path) -> Path | None:
    wav_files = sorted(capture_dir.glob('*.wav'), key=lambda path: path.stat().st_mtime)
    return wav_files[-1] if wav_files else None


def main() -> None:
    args = parse_args()
    dos_root = args.dos_root.expanduser().resolve()
    workspace_dir = dos_root / args.workspace_name
    qb_dir = dos_root / args.qb_dir.lower()
    if not qb_dir.exists():
        qb_dir = dos_root / args.qb_dir
    qb_exe = qb_dir / 'QB.EXE'

    if not workspace_dir.exists():
        raise SystemExit(f'DOS workspace not found: {workspace_dir}')
    if not qb_exe.exists():
        raise SystemExit(f'QB.EXE not found: {qb_exe}')

    sequence_map, runtime_names = load_play_catalog()
    capture_dir = (
        args.capture_dir.expanduser().resolve()
        if args.capture_dir
        else (DEFAULT_CAPTURE_ROOT / 'latest').resolve()
    )
    capture_dir.mkdir(parents=True, exist_ok=True)

    harness_path = workspace_dir / args.harness_name
    log_path = workspace_dir / args.log_name
    manifest_path = capture_dir / 'capture_manifest.json'
    conf_path = capture_dir / 'dosbox-x-audio.conf'
    manual_steps_path = capture_dir / DEFAULT_MANUAL_STEPS_NAME

    harness_path.write_text(
        build_basic_harness(runtime_names, sequence_map, args.workspace_name, args.log_name)
    )
    conf_path.write_text(
        build_dosbox_conf(dos_root, args.workspace_name, qb_dir.name, args.harness_name, capture_dir)
    )
    manifest_path.write_text(
        json.dumps(
            [
                {
                    'name': name,
                    'sequence': sequence_map[name],
                    'capture_sequence': force_foreground(sequence_map[name]),
                }
                for name in runtime_names
            ],
            indent=2,
        )
        + '\n'
    )
    manual_steps_path.write_text(
        build_manual_capture_steps(
            dos_root,
            qb_dir.name,
            args.workspace_name,
            args.harness_name,
            args.log_name,
            capture_dir,
        )
    )
    if log_path.exists():
        log_path.unlink()

    if not args.no_rust_wavs:
        export_rust_wavs(capture_dir)

    if args.generate_only:
        print(f'capture artifacts written to {capture_dir}')
        print(f'manual capture steps written to {manual_steps_path}')
        return

    if shutil.which(args.dosbox) is None:
        raise SystemExit(f'DOSBox executable not found on PATH: {args.dosbox}')

    before_capture = {path.name for path in capture_dir.glob('*.wav')}
    subprocess.run(
        [
            args.dosbox,
            '-defaultconf',
            '-conf',
            str(conf_path),
            '-fastlaunch',
            '-exit',
            '-time-limit',
            str(args.time_limit),
        ],
        check=True,
    )

    if log_path.exists():
        shutil.copy2(log_path, capture_dir / log_path.name)

    new_wavs = [path for path in capture_dir.glob('*.wav') if path.name not in before_capture]
    if len(new_wavs) == 1 and not args.keep_capture_raw_name:
        captured_wav = new_wavs[0]
        target_wav = capture_dir / DEFAULT_CAPTURE_FILENAME
        if target_wav.exists():
            target_wav.unlink()
        captured_wav.rename(target_wav)
    elif len(new_wavs) == 0 and newest_wav(capture_dir) is None:
        raise SystemExit('DOSBox-X completed but no WAV capture was produced')

    print(f'capture artifacts written to {capture_dir}')


if __name__ == '__main__':
    main()
