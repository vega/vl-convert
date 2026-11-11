"""Tests the built PyO3 extension, not a Python implementation of the backend."""
import asyncio
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


def test_native_axes_data_and_text(spec):
    tex = vlc.vegalite_to_pgfplots(spec)
    for text in (r"\begin{axis}", r"\addplot", "(1,2)", "(2,4)", "xlabel={x}"):
        assert text in tex
    for text in (r"\pgfpath", "includegraphics", "hide axis"):
        assert text not in tex
    assert tex == vlc.vegalite_to_pgfplots(json.dumps(spec))


def test_standalone(spec):
    tex = vlc.vegalite_to_pgfplots(spec, standalone=True)
    assert tex.startswith(r"\documentclass")
    assert tex.endswith("\\end{document}\n")


def test_math_is_opt_in(spec):
    spec["encoding"]["x"]["title"] = r"$E_{\mathrm{kin}}$"
    escaped = vlc.vegalite_to_pgfplots(spec)
    assert r"\$E\_\{" in escaped
    trusted = vlc.vegalite_to_pgfplots(spec, tex_labels=True)
    assert r"xlabel={$E_{\mathrm{kin}}$}" in trusted


def test_async_matches_sync(spec):
    import vl_convert.asyncio as avlc

    async def run():
        return await avlc.vegalite_to_pgfplots(spec)

    assert asyncio.run(run()) == vlc.vegalite_to_pgfplots(spec)


@pytest.mark.parametrize("invalid", [
    "not JSON",
    {"marks": []},
    {"mark": "line", "data": {"url": "file:///etc/passwd"}},
    {"mark": "line", "params": [{"name": "selection"}]},
    {"mark": "line", "transform": [{"calculate": "1+1", "as": "x"}]},
])
def test_unsupported_input_raises(invalid):
    with pytest.raises(ValueError):
        vlc.vegalite_to_pgfplots(invalid)


def test_failure_does_not_poison_worker(spec):
    with pytest.raises(ValueError):
        vlc.vegalite_to_pgfplots({"mark": "geoshape"})
    assert r"\addplot" in vlc.vegalite_to_pgfplots(spec)
