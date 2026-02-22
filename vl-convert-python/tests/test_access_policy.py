import asyncio
import base64
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
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


def make_vegalite_image_url_spec(url: str) -> dict:
    return {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"x": 0.5, "y": 0.5, "img": url}]},
        "mark": {"type": "image", "width": 20, "height": 20},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"},
            "url": {"field": "img", "type": "nominal"},
        },
    }


def _route(status: int, body: bytes = b"", headers: dict[str, str] | None = None) -> dict:
    return {"status": status, "body": body, "headers": headers or {}}


@contextmanager
def run_test_http_server(routes: dict[str, dict]):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = self.path.split("?", 1)[0]
            route = routes.get(path)
            if route is None:
                self.send_response(404)
                self.send_header("Content-Length", "0")
                self.end_headers()
                return

            status = int(route["status"])
            body = bytes(route["body"])
            headers = dict(route["headers"])
            self.send_response(status)
            for name, value in headers.items():
                self.send_header(name, value)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            if body:
                self.wfile.write(body)

        def log_message(self, format, *args):
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


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


def test_sync_filesystem_root_allows_under_root(tmp_path: Path):
    root = tmp_path / "root"
    root.mkdir()
    (root / "table.csv").write_text("a,b\n1,2\n", encoding="utf8")

    vlc.configure_converter(allow_http_access=False, filesystem_root=str(root))
    svg = vlc.vegalite_to_svg(
        make_vegalite_data_url_spec("table.csv"),
        vl_version="v5_16",
    )
    assert "<svg" in svg


def test_sync_allowed_base_urls_allows_http_and_normalizes_trailing_slash():
    with run_test_http_server(
        {
            "/allowed/data.csv": _route(
                200,
                b"a,b\n1,2\n",
                {"Content-Type": "text/csv"},
            )
        }
    ) as base_url:
        vlc.configure_converter(
            allow_http_access=True,
            allowed_base_urls=[f"{base_url}/allowed"],
        )
        svg = vlc.vega_to_svg(make_vega_data_url_spec(f"{base_url}/allowed/data.csv"))
        assert "<svg" in svg


def test_sync_redirect_to_disallowed_url_raises_permission_error():
    with run_test_http_server(
        {"/data.csv": _route(200, b"a,b\n1,2\n", {"Content-Type": "text/csv"})}
    ) as disallowed_base:
        with run_test_http_server(
            {
                "/redirect.csv": _route(
                    302, b"", {"Location": f"{disallowed_base}/data.csv"}
                )
            }
        ) as allowed_base:
            vlc.configure_converter(
                allow_http_access=True,
                allowed_base_urls=[allowed_base],
            )
            with pytest.raises(PermissionError):
                vlc.vega_to_svg(make_vega_data_url_spec(f"{allowed_base}/redirect.csv"))


def test_sync_redirect_is_allowed_without_allowlist():
    with run_test_http_server(
        {"/data.csv": _route(200, b"a,b\n1,2\n", {"Content-Type": "text/csv"})}
    ) as target_base:
        with run_test_http_server(
            {
                "/redirect.csv": _route(
                    302, b"", {"Location": f"{target_base}/data.csv"}
                )
            }
        ) as redirect_base:
            vlc.configure_converter(
                allow_http_access=True,
                allowed_base_urls=None,
            )
            svg = vlc.vega_to_svg(make_vega_data_url_spec(f"{redirect_base}/redirect.csv"))
            assert "<svg" in svg


def test_sync_per_request_allowlist_override_for_svg_rasterization():
    with run_test_http_server(
        {"/image.png": _route(200, PNG_1X1, {"Content-Type": "image/png"})}
    ) as base_url:
        vlc.configure_converter(
            allow_http_access=True,
            allowed_base_urls=["https://blocked.example/"],
        )
        jpeg = vlc.vegalite_to_jpeg(
            make_vegalite_image_url_spec(f"{base_url}/image.png"),
            vl_version="v5_16",
            allowed_base_urls=[base_url],
        )
        assert jpeg.startswith(b"\xff\xd8")


def test_sync_data_uri_pass_through_when_http_disabled():
    vlc.configure_converter(allow_http_access=False)

    svg = vlc.vega_to_svg(make_vega_data_url_spec("data:text/csv,a,b%0A1,2"))
    assert "<svg" in svg

    data_png = base64.b64encode(PNG_1X1).decode("ascii")
    image_svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">'
        f'<image href="data:image/png;base64,{data_png}" width="1" height="1"/>'
        "</svg>"
    )
    png = vlc.svg_to_png(image_svg)
    assert png.startswith(b"\x89PNG\r\n\x1a\n")


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


def test_asyncio_per_request_allowlist_and_data_uri_success():
    async def scenario():
        with run_test_http_server(
            {"/image.png": _route(200, PNG_1X1, {"Content-Type": "image/png"})}
        ) as base_url:
            await vlca.configure_converter(
                allow_http_access=True,
                allowed_base_urls=["https://blocked.example/"],
            )
            jpeg = await vlca.vegalite_to_jpeg(
                make_vegalite_image_url_spec(f"{base_url}/image.png"),
                vl_version="v5_16",
                allowed_base_urls=[base_url],
            )
            assert jpeg.startswith(b"\xff\xd8")

        await vlca.configure_converter(allow_http_access=False, allowed_base_urls=None)
        svg = await vlca.vega_to_svg(make_vega_data_url_spec("data:text/csv,a,b%0A1,2"))
        assert "<svg" in svg

    asyncio.run(scenario())
