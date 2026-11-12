"""Transport/adaptation checks that require neither TeX nor a native extension."""

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from gallery_catalog import exclusions, load_catalog
from gallery_check import check_results
from gallery_native import compare_coordinates, materialize_urls, randomized


class GalleryContracts(unittest.TestCase):
    def test_pinned_url_transport_preserves_data_and_original(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "data.json"
            source.write_text('[{"x":1,"url":"ordinary data"}]')
            registry = {
                "https://example.invalid/data": {
                    "path": str(source),
                    "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                }
            }
            spec = {
                "data": {
                    "url": "https://example.invalid/data",
                    "format": {"type": "json"},
                },
                "datasets": {"other": [{"url": "not a resource"}]},
            }
            before = json.dumps(spec)
            converted = materialize_urls(spec, registry)
            self.assertEqual(before, json.dumps(spec))
            self.assertEqual(converted["data"]["url"], str(source.resolve()))
            self.assertEqual(converted["data"]["format"], {"type": "json"})
            self.assertEqual(converted["datasets"], spec["datasets"])
            self.assertEqual(
                materialize_urls({"values": [{"url": "data"}]}, {}),
                {"values": [{"url": "data"}]},
            )
            source.write_text("[]")
            with self.assertRaisesRegex(ValueError, "content changed"):
                materialize_urls(spec, registry)

    def test_runtime_roundoff_is_bounded_without_ignoring_styles_or_structure(self):
        a = r"\addplot[color=blue] coordinates {(0.1,0.9950041652780257) (2,4)};"
        b = a.replace("0.9950041652780257", "0.9950041652780258")
        result = compare_coordinates(a, b)
        self.assertEqual(result["status"], "equivalent_coordinates")
        self.assertLess(result["max_absolute_difference"], 2e-16)
        self.assertEqual(compare_coordinates(a, a)["status"], "identical")
        for changed in (
            a.replace("blue", "red"),
            a.replace("(2,4)", "(2,4.000001)"),
            a.replace("(2,4)", "(4,2)"),
            a.replace("(2,4)", "(2,nan)"),
            a + r"\addlegendentry{changed}",
        ):
            with self.assertRaises(AssertionError):
                compare_coordinates(a, changed)

    def test_unregistered_resource_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Unpinned"):
            materialize_urls({"data": {"url": "https://example.invalid"}}, {})

    def test_randomized_results_are_not_claimed_byte_identical(self):
        self.assertTrue(randomized({"mark": "errorband"}))
        self.assertTrue(
            randomized({"transform": [{"calculate": "random()", "as": "j"}]})
        )
        self.assertTrue(randomized({"aggregate": "ci0", "field": "y"}))
        self.assertFalse(randomized({"mark": {"type": "errorbar", "extent": "stdev"}}))
        self.assertFalse(
            randomized({"mark": "errorband", "encoding": {"y2": {"field": "hi"}}})
        )
        self.assertFalse(randomized({"mark": "line"}))


class CatalogContracts(unittest.TestCase):
    def setUp(self):
        self.catalog = [{"name": "line"}, {"name": "map"}]
        self.raw = json.dumps({"examples": self.catalog}).encode()
        self.baseline = {
            "catalog_sha256": hashlib.sha256(self.raw).hexdigest(),
            "accepted": ["line"],
            "excluded": {"excluded_map": ["map"], "excluded_interactive": []},
        }
        self.rows = [
            {
                "id": "line",
                "status": "tex_generated",
                "reference": "rendered",
                "latex": "passed",
                "interfaces": "identical",
            },
            {"id": "map", "status": "excluded_map"},
        ]

    def test_catalog_is_content_pinned(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.json"
            path.write_bytes(self.raw)
            self.assertEqual(load_catalog(path, self.baseline), self.catalog)
            path.write_bytes(self.raw + b" ")
            with self.assertRaisesRegex(ValueError, "does not match"):
                load_catalog(path, self.baseline)

    def test_policy_rejects_unknown_overlapping_or_duplicate_cases(self):
        bad_policies = []
        for accepted in (["unknown"], ["map"], ["line", "line"]):
            bad_policies.append({**self.baseline, "accepted": accepted})
        bad_policies.append(
            {**self.baseline, "excluded": {"excluded_map": ["map", "map"]}}
        )
        bad_policies.append({**self.baseline, "excluded": {"unreviewed": ["map"]}})
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.json"
            path.write_bytes(self.raw)
            for policy in bad_policies:
                with self.subTest(policy=policy), self.assertRaises(ValueError):
                    load_catalog(path, policy)

    def test_duplicate_catalog_entries_fail(self):
        raw = json.dumps({"examples": self.catalog + [self.catalog[0]]}).encode()
        policy = {**self.baseline, "catalog_sha256": hashlib.sha256(raw).hexdigest()}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.json"
            path.write_bytes(raw)
            with self.assertRaisesRegex(ValueError, "Duplicate example"):
                load_catalog(path, policy)

    def test_complete_report_passes(self):
        self.assertEqual(exclusions(self.baseline), {"map": "excluded_map"})
        self.assertEqual(
            check_results(self.rows, self.catalog, self.baseline, True), []
        )

    def test_exclusion_cannot_hide_a_regression(self):
        rows = copy.deepcopy(self.rows)
        rows[0]["status"] = "excluded_interactive"
        self.assertTrue(check_results(rows, self.catalog, self.baseline, True))
        rows = copy.deepcopy(self.rows)
        rows[1]["status"] = "excluded_interactive"
        self.assertTrue(check_results(rows, self.catalog, self.baseline, True))

    def test_missing_duplicate_and_unknown_results_fail(self):
        for rows in (
            self.rows[:1],
            self.rows + [self.rows[0]],
            self.rows + [{"id": "unknown", "status": "excluded_map"}],
        ):
            with self.subTest(rows=rows):
                self.assertTrue(check_results(rows, self.catalog, self.baseline, True))

    def test_reference_latex_and_native_checks_are_preserved(self):
        for key in ("reference", "latex", "interfaces"):
            rows = copy.deepcopy(self.rows)
            del rows[0][key]
            with self.subTest(key=key):
                self.assertTrue(check_results(rows, self.catalog, self.baseline, True))


if __name__ == "__main__":
    unittest.main()
