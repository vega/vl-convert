"""Native-interface tests: require the patched extension, never skip silently."""
import json

import pytest
import vl_convert as vlc


@pytest.fixture
def spec():
    return {
        "data": {"values": [{"x": 1, "y": 2}, {"x": 2, "y": 4}]},
        "mark": "line",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"},
        },
    }


def test_api_is_present():
    for name in ("vegalite_to_pgfplots", "vega_to_pgfplots", "svg_to_pgfplots"):
        assert callable(getattr(vlc, name))


def test_dict_and_json_are_equivalent(spec):
    assert vlc.vegalite_to_pgfplots(spec) == vlc.vegalite_to_pgfplots(json.dumps(spec))


def test_compilation_is_shared(spec):
    lite = vlc.vegalite_to_pgfplots(spec)
    vega = vlc.vegalite_to_vega(spec)
    assert lite == vlc.vega_to_pgfplots(vega)


def test_standalone(spec):
    text = vlc.vegalite_to_pgfplots(spec, standalone=True)
    assert text.startswith("\\documentclass")
    assert text.endswith("\\end{document}\n")


def test_invalid_svg_raises():
    with pytest.raises(ValueError):
        vlc.svg_to_pgfplots("not XML")


def test_raw_tex_is_never_executed():
    text = vlc.svg_to_pgfplots(
        '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40">'
        '<text x="1" y="20">\\input{secret}</text></svg>'
    )
    assert "\\input" not in text


@pytest.mark.parametrize("body", [
    '<image href="file:///etc/passwd"/>',
    '<linearGradient id="g"/>',
    '<foreignObject/>',
])
def test_unsupported_has_actionable_error(body):
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">{body}</svg>'
    with pytest.raises(ValueError, match="unsupported"):
        vlc.svg_to_pgfplots(svg)
