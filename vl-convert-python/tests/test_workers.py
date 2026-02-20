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
    original = vlc.get_num_workers()
    vlc.set_num_workers(1)
    try:
        yield
    finally:
        vlc.set_num_workers(original)


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
