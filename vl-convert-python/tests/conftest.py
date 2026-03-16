"""Pytest configuration for vl-convert-python tests."""

import pytest
from pathlib import Path
import vl_convert as vlc


def pytest_addoption(parser):
    parser.addoption(
        "--update-baselines",
        action="store_true",
        default=False,
        help="Update HTML screenshot baselines instead of comparing",
    )


@pytest.fixture
def update_baselines(request):
    return request.config.getoption("--update-baselines")


def pytest_configure(config):
    """Register test fonts before any tests run."""
    root_dir = Path(__file__).parent.parent.parent
    fonts_dir = root_dir / "vl-convert-rs" / "tests" / "fonts"
    if fonts_dir.exists():
        vlc.register_font_directory(str(fonts_dir))
