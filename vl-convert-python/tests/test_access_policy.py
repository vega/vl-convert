import asyncio
from pathlib import Path

import pytest
import vl_convert as vlc
import vl_convert.asyncio as vlca

PNG_1X1 = bytes(
    [
        137,
        80,
        78,
        71,
        13,
        10,
        26,
        10,
        0,
        0,
        0,
        13,
        73,
        72,
        68,
        82,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        1,
        8,
        4,
        0,
        0,
        0,
        181,
        28,
        12,
        2,
        0,
        0,
        0,
        11,
        73,
        68,
        65,
        84,
        120,
        218,
        99,
        252,
        255,
        15,
        0,
        2,
        3,
        1,
        128,
        179,
        248,
        175,
        217,
        0,
        0,
        0,
        0,
        73,
        69,
        78,
        68,
        174,
        66,
        96,
        130,
    ]
)


def make_vega_data_url_spec(url: str) -> dict:
    return {
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 20,
        "height": 20,
        "data": [{"name": "table", "url": url, "format": {"type": "csv"}}],
        "scales": [
            {
                "name": "x",
                "type": "linear",
                "range": "width",
                "domain": {"data": "table", "field": "a"},
            },
            {
                "name": "y",
                "type": "linear",
                "range": "height",
                "domain": {"data": "table", "field": "b"},
            },
        ],
        "marks": [
            {
                "type": "symbol",
                "from": {"data": "table"},
                "encode": {
                    "enter": {
                        "x": {"scale": "x", "field": "a"},
                        "y": {"scale": "y", "field": "b"},
                    }
                },
            }
        ],
    }


def make_vegalite_data_url_spec(url: str) -> dict:
    return {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"url": url},
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"},
        },
    }


@pytest.fixture(autouse=True)
def reset_converter_config():
    vlc.configure_converter(
        num_workers=1,
        allow_http_access=True,
        filesystem_root=None,
        allowed_base_urls=None,
    )
    try:
        yield
    finally:
        vlc.configure_converter(
            num_workers=1,
            allow_http_access=True,
            filesystem_root=None,
            allowed_base_urls=None,
        )


def test_sync_denied_http_and_filesystem_access(tmp_path: Path):
    vlc.configure_converter(allow_http_access=False)

    with pytest.raises(PermissionError):
        vlc.vega_to_svg(make_vega_data_url_spec("https://example.com/data.csv"))

    root = tmp_path / "root"
    root.mkdir()
    outside_csv = tmp_path / "outside.csv"
    outside_csv.write_text("a,b\n1,2\n", encoding="utf8")

    vlc.configure_converter(
        allow_http_access=False,
        filesystem_root=str(root),
    )

    with pytest.raises(PermissionError):
        vlc.vegalite_to_svg(
            make_vegalite_data_url_spec(outside_csv.resolve().as_uri()),
            vl_version="v5_16",
        )


def test_sync_svg_helpers_raise_permission_error(tmp_path: Path):
    vlc.configure_converter(allow_http_access=False)
    http_svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">'
        '<image href="https://example.com/image.png" width="1" height="1"/>'
        "</svg>"
    )
    with pytest.raises(PermissionError):
        vlc.svg_to_png(http_svg)

    local_png = tmp_path / "local.png"
    local_png.write_bytes(PNG_1X1)
    local_svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">'
        f'<image href="{local_png.resolve().as_uri()}" width="1" height="1"/>'
        "</svg>"
    )
    with pytest.raises(PermissionError):
        vlc.svg_to_png(local_svg)


def test_asyncio_denied_access_raises_permission_error(tmp_path: Path):
    async def scenario():
        await vlca.configure_converter(allow_http_access=False)

        with pytest.raises(PermissionError):
            await vlca.vega_to_svg(make_vega_data_url_spec("https://example.com/data.csv"))

        root = tmp_path / "root"
        root.mkdir()
        outside_csv = tmp_path / "outside.csv"
        outside_csv.write_text("a,b\n1,2\n", encoding="utf8")

        await vlca.configure_converter(
            allow_http_access=False,
            filesystem_root=str(root),
        )

        with pytest.raises(PermissionError):
            await vlca.vegalite_to_svg(
                make_vegalite_data_url_spec(outside_csv.resolve().as_uri()),
                vl_version="v5_16",
            )

        await vlca.configure_converter(allow_http_access=False)

        http_svg = (
            '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">'
            '<image href="https://example.com/image.png" width="1" height="1"/>'
            "</svg>"
        )
        with pytest.raises(PermissionError):
            await vlca.svg_to_png(http_svg)

        local_png = tmp_path / "local.png"
        local_png.write_bytes(PNG_1X1)
        local_svg = (
            '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">'
            f'<image href="{local_png.resolve().as_uri()}" width="1" height="1"/>'
            "</svg>"
        )
        with pytest.raises(PermissionError):
            await vlca.svg_to_png(local_svg)

    asyncio.run(scenario())
