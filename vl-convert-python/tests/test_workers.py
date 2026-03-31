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


def test_load_config_from_jsonc_file(tmp_path):
    # Write a JSONC config with comments and trailing commas that registers a
    # custom theme and sets missing_fonts policy.
    config_file = tmp_path / "vlc-config.jsonc"
    config_file.write_text(
        """{
    // Custom config for load_config test
    "missing_fonts": "warn", // override default
    "themes": {
        "my-test-theme": {
            "background": "#abcdef",
        },
    },
}"""
    )

    vlc.load_config(str(config_file))

    config = vlc.get_config()
    assert config["missing_fonts"] == "warn"
    themes = vlc.get_themes()
    assert "my-test-theme" in themes
    assert themes["my-test-theme"]["background"] == "#abcdef"
    # Built-in themes are still present
    assert "dark" in themes


def test_load_config_resets_prior_configure_state(tmp_path):
    # configure() patches are blown away by a subsequent load_config().
    vlc.configure(missing_fonts="error", auto_google_fonts=True)
    assert vlc.get_config()["missing_fonts"] == "error"
    assert vlc.get_config()["auto_google_fonts"] is True

    config_file = tmp_path / "vlc-config.jsonc"
    config_file.write_text('{"missing_fonts": "warn"}')
    vlc.load_config(str(config_file))

    config = vlc.get_config()
    assert config["missing_fonts"] == "warn"
    # auto_google_fonts was not in the file → reset to default (False)
    assert config["auto_google_fonts"] is False


def test_load_config_then_configure_override(tmp_path):
    # load_config establishes baseline, configure patches on top.
    config_file = tmp_path / "vlc-config.jsonc"
    config_file.write_text('{"missing_fonts": "warn"}')

    vlc.load_config(str(config_file))
    assert vlc.get_config()["missing_fonts"] == "warn"

    # configure overrides the file-loaded value
    vlc.configure(missing_fonts="error")
    assert vlc.get_config()["missing_fonts"] == "error"


def test_load_config_invalid_path_raises():
    with pytest.raises(Exception):
        vlc.load_config("/nonexistent/path/vlc-config.jsonc")
