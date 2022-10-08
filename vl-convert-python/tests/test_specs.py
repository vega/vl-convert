from pathlib import Path
from vl_convert import VlConverter
import pytest

tests_dir = Path(__file__).parent
root_dir = tests_dir.parent.parent
specs_dir = root_dir / "vl-convert-rs" / "tests" / "vl-specs"


def load_vl_spec(name):
    spec_path = specs_dir / f"{name}.vl.json"
    with open(spec_path, "rt") as f:
        spec_str = f.read()
    return spec_str


def load_expected_vg_spec(name, vl_version, pretty):
    filename = f"{name}.vg.pretty.json" if pretty else f"{name}.vg.json"
    spec_path = specs_dir / "expected" / vl_version / filename
    if spec_path.exists():
        with spec_path.open("rt", encoding="utf8") as f:
            return f.read()
    else:
        return None


@pytest.mark.parametrize("name", ["circle_binned", "seattle-weather"])
@pytest.mark.parametrize(
    "vl_version", ["v4_17", "v5_0", "v5_1", "v5_2", "v5_3", "v5_4", "v5_5"]
)
@pytest.mark.parametrize("pretty", [True, False])
def test_reference_specs(name, vl_version, pretty):
    vl_spec = load_vl_spec(name)
    expected_vg_spec = load_expected_vg_spec(name, vl_version, pretty)

    # Initialize converter
    converter = VlConverter()

    if expected_vg_spec is None:
        with pytest.raises(ValueError):
            converter.vegalite_to_vega(vl_spec, vl_version=vl_version, pretty=pretty)
    else:
        vg_spec = converter.vegalite_to_vega(
            vl_spec, vl_version=vl_version, pretty=pretty
        )
        assert expected_vg_spec == vg_spec
