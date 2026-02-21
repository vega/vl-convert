import concurrent.futures

import pytest
import vl_convert as vlc


SIMPLE_VL_SPEC = {
    "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "a", "type": "nominal"},
        "y": {"field": "b", "type": "quantitative"},
    },
}


@pytest.fixture(autouse=True)
def reset_worker_count():
    original = vlc.get_converter_config()
    vlc.configure_converter(num_workers=1)
    try:
        yield
    finally:
        vlc.configure_converter(**original)


def test_get_num_workers_default_one():
    assert vlc.get_num_workers() == 1


def test_set_num_workers_rejects_zero():
    with pytest.raises(ValueError):
        vlc.set_num_workers(0)


def test_parallel_threadpool_conversions_with_configured_workers():
    vlc.set_num_workers(4)

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [
            executor.submit(vlc.vegalite_to_svg, SIMPLE_VL_SPEC, "v5_16")
            for _ in range(16)
        ]
        svg_results = [future.result(timeout=30) for future in futures]

    assert len(svg_results) == 16
    assert all(svg.lstrip().startswith("<svg") for svg in svg_results)


def test_warm_up_workers_then_parallel_conversions():
    vlc.set_num_workers(4)
    vlc.warm_up_workers()

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [
            executor.submit(vlc.vegalite_to_svg, SIMPLE_VL_SPEC, "v5_16")
            for _ in range(16)
        ]
        svg_results = [future.result(timeout=30) for future in futures]

    assert len(svg_results) == 16
    assert all(svg.lstrip().startswith("<svg") for svg in svg_results)


def test_reconfigure_workers_while_requests_are_running():
    vlc.set_num_workers(4)

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [
            executor.submit(vlc.vegalite_to_svg, SIMPLE_VL_SPEC, "v5_16")
            for _ in range(24)
        ]
        vlc.set_num_workers(2)
        vlc.set_num_workers(3)
        svg_results = [future.result(timeout=30) for future in futures]

    assert len(svg_results) == 24
    assert all(svg.lstrip().startswith("<svg") for svg in svg_results)


def test_configure_converter_round_trip(tmp_path):
    root = tmp_path / "root"
    root.mkdir()

    vlc.configure_converter(
        num_workers=2,
        allow_http_access=False,
        filesystem_root=str(root),
        allowed_base_urls=None,
    )

    config = vlc.get_converter_config()
    assert config["num_workers"] == 2
    assert config["allow_http_access"] is False
    assert config["filesystem_root"] == str(root.resolve())
    assert config["allowed_base_urls"] is None


def test_set_num_workers_preserves_access_policy(tmp_path):
    root = tmp_path / "root"
    root.mkdir()

    vlc.configure_converter(
        num_workers=2,
        allow_http_access=False,
        filesystem_root=str(root),
        allowed_base_urls=None,
    )
    vlc.set_num_workers(3)
    config = vlc.get_converter_config()

    assert config["num_workers"] == 3
    assert config["allow_http_access"] is False
    assert config["filesystem_root"] == str(root.resolve())


def test_configure_converter_noop_when_called_without_args():
    vlc.configure_converter(
        num_workers=2,
        allow_http_access=True,
        filesystem_root=None,
        allowed_base_urls=["https://example.com/"],
    )
    before = vlc.get_converter_config()
    vlc.configure_converter()
    after = vlc.get_converter_config()
    assert after == before
