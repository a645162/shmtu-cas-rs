#!/usr/bin/env python3
"""Run the shmtu-ocr-gui (egui) application."""

import subprocess
import sys
from pathlib import Path

gui_dir = Path(__file__).parent / "ocr" / "shmtu-ocr-gui"

if not gui_dir.exists():
    print(f"Error: GUI project not found at {gui_dir}", file=sys.stderr)
    sys.exit(1)

sys.exit(subprocess.call(["cargo", "run"], cwd=gui_dir))
