import asyncio
import inspect

import pytest
import vl_convert.asyncio as vlca

import vl_convert as vlc


@pytest.mark.parametrize("spec_kind", ["vega", "vegalite"])
@pytest.mark.parametrize("use_async", [False, True])
def test_font_subsetting_override(spec_kind, use_async):
    spec = {
        "data": {"values": [{"label": "Hello"}]},
        "mark": {"type": "text", "font": "Caveat"},
        "encoding": {"text": {"field": "label"}},
    }
    if spec_kind == "vega":
        spec = vlc.vegalite_to_vega(spec)
    get_fonts = getattr(vlca if use_async else vlc, f"{spec_kind}_fonts")
    parameter = inspect.signature(get_fonts).parameters["subset_fonts"]
    assert parameter.kind == inspect.Parameter.KEYWORD_ONLY
    assert parameter.default is None

    async def font_face_css(**kwargs):
        result = get_fonts(spec, include_font_face=True, **kwargs)
        if use_async:
            result = await result
        return result[0]["variants"][0]["font_face"]

    async def scenario():
        for configured_subset in (False, True):
            vlc.configure(embed_local_fonts=True, subset_fonts=configured_subset)
            inherited = await font_face_css()
            assert inherited == await font_face_css(subset_fonts=None)
            full = await font_face_css(subset_fonts=False)
            subset = await font_face_css(subset_fonts=True)
            assert 0 < len(subset) < len(full)
            assert inherited == (subset if configured_subset else full)
            assert vlc.get_config()["subset_fonts"] is configured_subset

    asyncio.run(scenario())
