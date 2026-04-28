"""Tests for per-call VgOpts/VlOpts arguments on conversion methods.

Verify that `background`, `width`, `height`, and `config` (vega
methods) reach the converter and influence output.
"""

import re

import pytest

import vl_convert as vlc


@pytest.fixture
def vl_bar_spec():
    return {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 28}, {"a": "B", "b": 55}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"},
        },
    }


@pytest.fixture
def vl_scatter_spec():
    """Quantitative-axis spec. `width`/`height` overrides propagate
    deterministically (no band-size derivation)."""
    return {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"x": 1, "y": 2}, {"x": 3, "y": 5}]},
        "mark": "point",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"},
        },
    }


@pytest.fixture
def vg_bar_spec(vl_bar_spec):
    return vlc.vegalite_to_vega(vl_bar_spec)


@pytest.fixture
def vg_scatter_spec(vl_scatter_spec):
    return vlc.vegalite_to_vega(vl_scatter_spec)


def _svg_root_width(svg: str) -> int:
    match = re.search(r'<svg[^>]*\swidth="(\d+)"', svg)
    assert match, f"no width attribute on root <svg>: {svg[:200]}"
    return int(match.group(1))


def _svg_root_height(svg: str) -> int:
    match = re.search(r'<svg[^>]*\sheight="(\d+)"', svg)
    assert match, f"no height attribute on root <svg>: {svg[:200]}"
    return int(match.group(1))


class TestBackgroundOverride:
    def test_vegalite_to_svg(self, vl_bar_spec):
        svg = vlc.vegalite_to_svg(vl_bar_spec, background="#ff0000")
        assert "#ff0000" in svg

    def test_vega_to_svg(self, vg_bar_spec):
        svg = vlc.vega_to_svg(vg_bar_spec, background="#00ff00")
        assert "#00ff00" in svg


class TestWidthHeightOverride:
    def test_vegalite_to_svg_width_height(self, vl_scatter_spec):
        svg_default = vlc.vegalite_to_svg(vl_scatter_spec)
        svg_override = vlc.vegalite_to_svg(vl_scatter_spec, width=400, height=100)
        assert _svg_root_width(svg_override) > _svg_root_width(svg_default)
        assert _svg_root_height(svg_override) < _svg_root_height(svg_default)

    def test_vega_to_svg_width_height(self, vg_scatter_spec):
        svg_default = vlc.vega_to_svg(vg_scatter_spec)
        svg_override = vlc.vega_to_svg(vg_scatter_spec, width=400, height=100)
        assert _svg_root_width(svg_override) > _svg_root_width(svg_default)
        assert _svg_root_height(svg_override) < _svg_root_height(svg_default)


class TestVegaConfigMerge:
    def test_vega_to_svg_config_overrides_axis_color(self, vg_bar_spec):
        # No per-call config: default axis stroke is grey.
        svg_default = vlc.vega_to_svg(vg_bar_spec)
        # With per-call config: axis domain color set to magenta.
        svg_override = vlc.vega_to_svg(
            vg_bar_spec, config={"axis": {"domainColor": "#ff00ff"}}
        )
        assert "#ff00ff" not in svg_default
        assert "#ff00ff" in svg_override
