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


@pytest.fixture(autouse=True)
def reset_config_to_test_baseline():
    """Reset the converter config to a deterministic baseline before every
    test.

    Rationale: individual tests call `vlc.configure(...)` to flip specific
    flags (`auto_google_fonts=True`, `allowed_base_urls=[]`,
    `embed_local_fonts=True`, etc.). Without a reset between tests, one
    test's overrides leak into the next and cause spurious failures — for
    example, `test_pacifico_bundle` leaving `auto_google_fonts=True` on
    would change the SVG output of any downstream spec that references a
    Google font, drifting it off the golden baseline.

    The baseline enables `allowed_base_urls=["http:", "https:"]` (most
    vendored specs reference `vega.github.io` / `raw.githubusercontent.com`
    URLs and the secure default blocks all network data), and clears
    every other `configure()`-settable field back to `VlcConfig::default()`.

    `font_directories` is NOT reset — the `pytest_configure` hook already
    wired the test fonts directory into the tracked config, and our
    `register_font_directory` keeps it durable across rebuilds.
    """
    vlc.configure(
        allowed_base_urls=["http:", "https:"],
        auto_google_fonts=False,
        embed_local_fonts=False,
        subset_fonts=True,
        missing_fonts="fallback",
        allow_google_fonts=False,
        allow_per_request_plugins=False,
        base_url=True,
        gc_after_conversion=False,
        google_fonts=[],
        vega_plugins=[],
        plugin_import_domains=[],
        per_request_plugin_import_domains=[],
        themes={},
        default_theme=None,
        default_format_locale=None,
        default_time_format_locale=None,
    )
    yield
