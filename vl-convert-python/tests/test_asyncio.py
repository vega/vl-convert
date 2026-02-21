import asyncio

import pytest
import vl_convert as vlc
import vl_convert.asyncio as vlca


SIMPLE_VL_SPEC = {
    "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "a", "type": "nominal"},
        "y": {"field": "b", "type": "quantitative"},
    },
}


def run(coro):
    return asyncio.run(coro)


def public_callable_names(module):
    return {
        name
        for name in dir(module)
        if not name.startswith("_") and callable(getattr(module, name))
    }


@pytest.fixture(autouse=True)
def reset_worker_count():
    original = vlc.get_converter_config()
    vlc.configure_converter(num_workers=1)
    try:
        yield
    finally:
        vlc.configure_converter(**original)


def test_asyncio_namespace_import_and_expected_attributes():
    assert hasattr(vlca, "vegalite_to_svg")
    assert hasattr(vlca, "vega_to_scenegraph")
    assert hasattr(vlca, "warm_up_workers")


def test_asyncio_module_has_sync_callable_parity():
    sync_callables = public_callable_names(vlc)
    async_callables = public_callable_names(vlca)
    assert async_callables == sync_callables


def test_asyncio_functions_have_docstrings():
    for name in public_callable_names(vlc):
        doc = getattr(vlca, name).__doc__
        assert isinstance(doc, str)
        assert doc.strip()
        assert f"vl_convert.{name}" in doc


def test_asyncio_smoke_and_sync_parity_shapes():
    async def scenario():
        vega = await vlca.vegalite_to_vega(SIMPLE_VL_SPEC, "v5_16")
        assert isinstance(vega, dict)

        svg = await vlca.vega_to_svg(vega)
        assert svg.lstrip().startswith("<svg")

        msgpack_scenegraph = await vlca.vega_to_scenegraph(vega, format="msgpack")
        assert isinstance(msgpack_scenegraph, bytes)

        version = await vlca.get_vega_version()
        assert version == vlc.get_vega_version()

    run(scenario())


def test_asyncio_parallel_gather_with_workers():
    async def scenario():
        await vlca.set_num_workers(4)
        await vlca.warm_up_workers()

        results = await asyncio.gather(
            *[vlca.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16") for _ in range(16)]
        )

        assert len(results) == 16
        assert all(svg.lstrip().startswith("<svg") for svg in results)

    run(scenario())


def test_asyncio_worker_lifecycle_calls():
    async def scenario():
        await vlca.set_num_workers(3)
        assert await vlca.get_num_workers() == 3
        await vlca.warm_up_workers()
        svg = await vlca.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16")
        assert svg.lstrip().startswith("<svg")

    run(scenario())


def test_asyncio_configure_converter_round_trip(tmp_path):
    async def scenario():
        root = tmp_path / "root"
        root.mkdir()
        await vlca.configure_converter(
            num_workers=2,
            allow_http_access=False,
            filesystem_root=str(root),
        )
        config = await vlca.get_converter_config()
        assert config["num_workers"] == 2
        assert config["allow_http_access"] is False
        assert config["filesystem_root"] == str(root.resolve())

    run(scenario())


def test_asyncio_html_conversions():
    async def scenario():
        vega = await vlca.vegalite_to_vega(SIMPLE_VL_SPEC, "v5_16")
        vega_html = await vlca.vega_to_html(vega, bundle=True)
        assert vega_html.startswith("<!DOCTYPE html>")

        vegalite_html = await vlca.vegalite_to_html(SIMPLE_VL_SPEC, "v5_16", bundle=True)
        assert vegalite_html.startswith("<!DOCTYPE html>")

    run(scenario())


def test_asyncio_javascript_bundle_custom_snippet():
    async def scenario():
        bundle = await vlca.javascript_bundle(
            "window.__vlcBundleMarker = 'ok';", vl_version="v5_16"
        )
        assert "__vlcBundleMarker" in bundle

    run(scenario())


def test_asyncio_cancellation_does_not_poison_followup_requests():
    async def scenario():
        await vlca.set_num_workers(4)
        await vlca.warm_up_workers()

        task = asyncio.ensure_future(vlca.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16"))
        assert task.cancel()

        with pytest.raises(asyncio.CancelledError):
            await task

        svg = await vlca.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16")
        assert svg.lstrip().startswith("<svg")

    run(scenario())


def test_sync_invalid_config_raises_value_error():
    with pytest.raises(ValueError):
        vlc.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16", config="{bad json")


def test_asyncio_invalid_config_raises_value_error():
    async def scenario():
        with pytest.raises(ValueError):
            await vlca.vegalite_to_svg(SIMPLE_VL_SPEC, "v5_16", config="{bad json")

    run(scenario())
