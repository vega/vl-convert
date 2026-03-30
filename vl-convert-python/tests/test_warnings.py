import logging
import warnings

import vl_convert as vlc
from inline_snapshot import snapshot


BRACKETS_SPEC = {
    "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
    "data": {
        "values": [
            {"Force in [N]": 10, "Travel in [inches]": 1},
            {"Force in [N]": 20, "Travel in [inches]": 2},
        ]
    },
    "mark": "line",
    "encoding": {
        "x": {"field": "Travel in [inches]", "type": "quantitative"},
        "y": {"field": "Force in [N]", "type": "quantitative"},
    },
}

LOG_SCALE_SPEC = {
    "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
    "width": "container",
    "data": {"values": [{"a": "A", "v": 100}, {"a": "B", "v": 200}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "a", "type": "nominal"},
        "y": {"field": "v", "type": "quantitative", "scale": {"type": "log"}},
    },
}


def test_vega_runtime_warnings_captured(caplog):
    """Vega View expression errors (e.g. brackets in field names) are surfaced."""
    with caplog.at_level(logging.WARNING, logger="vl_convert"):
        vlc.vegalite_to_svg(BRACKETS_SPEC)

    warning_messages = [r.message for r in caplog.records if r.levelno >= logging.WARNING]
    assert warning_messages == snapshot(
        [
            'Infinite extent for field "Travel in .inches": [Infinity, -Infinity]',
            'Infinite extent for field "Force in .N": [Infinity, -Infinity]',
        ]
    )


def test_vl_compilation_warnings_captured(caplog):
    """Vega-Lite compilation warnings (e.g. log scale domain) are surfaced."""
    with caplog.at_level(logging.WARNING, logger="vl_convert"):
        vlc.vegalite_to_svg(LOG_SCALE_SPEC)

    warning_messages = [r.message for r in caplog.records if r.levelno >= logging.WARNING]
    assert warning_messages == snapshot(
        [
            "Can not resolve event source: window",
            "Log scale domain includes zero: [0,200]",
        ]
    )


def test_no_warnings_for_valid_spec(caplog):
    """A valid spec with no issues produces no warnings."""
    spec = {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 28}, {"a": "B", "b": 55}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "b", "type": "quantitative"},
            "y": {"field": "a", "type": "nominal"},
        },
    }
    with caplog.at_level(logging.WARNING, logger="vl_convert"):
        vlc.vegalite_to_svg(spec)

    vl_warnings = [
        r.message
        for r in caplog.records
        if r.levelno >= logging.WARNING and "vl_convert" in r.name
    ]
    assert len(vl_warnings) == 0, f"Unexpected warnings: {vl_warnings}"


def test_warnings_not_duplicated_across_calls(caplog):
    """Each conversion clears warnings from the previous call."""
    with caplog.at_level(logging.WARNING, logger="vl_convert"):
        vlc.vegalite_to_svg(LOG_SCALE_SPEC)
        first_count = sum(
            1 for r in caplog.records
            if "Log scale" in r.message and r.levelno >= logging.WARNING
        )

        caplog.clear()
        vlc.vegalite_to_svg(LOG_SCALE_SPEC)
        second_count = sum(
            1 for r in caplog.records
            if "Log scale" in r.message and r.levelno >= logging.WARNING
        )

    assert first_count == second_count, (
        f"Warning counts differ between calls: {first_count} vs {second_count}"
    )


def test_show_warnings_deprecation():
    """show_warnings=True emits a DeprecationWarning."""
    spec = {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": 1, "b": 2}]},
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"},
        },
    }
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        vlc.vegalite_to_svg(spec, show_warnings=True)

    deprecation_warnings = [x for x in w if issubclass(x.category, DeprecationWarning)]
    assert len(deprecation_warnings) >= 1, "Expected DeprecationWarning for show_warnings"
    assert "show_warnings is deprecated" in str(deprecation_warnings[0].message)
