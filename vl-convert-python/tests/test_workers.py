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
    original = vlc.get_config()
    vlc.configure(num_workers=1)
    try:
        yield
    finally:
        vlc.configure(**original)


def test_get_config_reports_default_num_workers():
    assert vlc.get_config()["num_workers"] == 1


def test_configure_rejects_zero_num_workers():
    with pytest.raises(ValueError):
        vlc.configure(num_workers=0)


def test_configure_accepts_empty_allowed_base_urls():
    vlc.configure(allowed_base_urls=[])
    config = vlc.get_config()
    assert config["allowed_base_urls"] == []


def test_parallel_threadpool_conversions_with_configured_workers():
    vlc.configure(num_workers=4)

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [
            executor.submit(vlc.vegalite_to_svg, SIMPLE_VL_SPEC, "v5_16")
            for _ in range(16)
        ]
        svg_results = [future.result(timeout=30) for future in futures]

    assert len(svg_results) == 16
    assert all(svg.lstrip().startswith("<svg") for svg in svg_results)


def test_warm_up_workers_then_parallel_conversions():
    vlc.configure(num_workers=4)
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
    vlc.configure(num_workers=4)

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        futures = [
            executor.submit(vlc.vegalite_to_svg, SIMPLE_VL_SPEC, "v5_16")
            for _ in range(24)
        ]
        vlc.configure(num_workers=2)
        vlc.configure(num_workers=3)
        svg_results = [future.result(timeout=30) for future in futures]

    assert len(svg_results) == 24
    assert all(svg.lstrip().startswith("<svg") for svg in svg_results)


def test_configure_round_trip(tmp_path):
    root = tmp_path / "root"
    root.mkdir()

    vlc.configure(
        num_workers=2,
        base_url=str(root),
        allowed_base_urls=[str(root) + "/"],
    )

    config = vlc.get_config()
    assert config["num_workers"] == 2
    assert config["base_url"] == str(root)
    assert config["allowed_base_urls"] == [str(root) + "/"]


def test_configure_num_workers_preserves_access_policy(tmp_path):
    root = tmp_path / "root"
    root.mkdir()

    vlc.configure(
        num_workers=2,
        base_url=str(root),
        allowed_base_urls=[str(root) + "/"],
    )
    vlc.configure(num_workers=3)
    config = vlc.get_config()

    assert config["num_workers"] == 3
    assert config["base_url"] == str(root)
    assert config["allowed_base_urls"] == [str(root) + "/"]


def test_configure_noop_when_called_without_args():
    vlc.configure(
        num_workers=2,
        allowed_base_urls=["https://example.com/"],
    )
    before = vlc.get_config()
    vlc.configure()
    after = vlc.get_config()
    assert after == before
