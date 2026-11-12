"""Transport/adaptation checks that require neither TeX nor a native extension."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

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


if __name__ == "__main__":
    unittest.main()
