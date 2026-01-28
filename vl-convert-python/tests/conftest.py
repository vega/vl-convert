"""Pytest configuration for vl-convert-python tests."""

import pytest
from pathlib import Path
import vl_convert as vlc


def pytest_configure(config):
    """Register test fonts before any tests run."""
    root_dir = Path(__file__).parent.parent.parent
    fonts_dir = root_dir / "vl-convert-rs" / "tests" / "fonts"
    if fonts_dir.exists():
        vlc.register_font_directory(str(fonts_dir))
