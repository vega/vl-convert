from pathlib import Path
import vl_convert as vlc
import pytest

tests_dir = Path(__file__).parent
root_dir = tests_dir.parent.parent
specs_dir = root_dir / "vl-convert-rs" / "tests" / "vl-specs"
fonts_dir = root_dir / "vl-convert-rs" / "tests" / "fonts"


def setup_module(module):
    vlc.register_font_directory(str(fonts_dir))


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


def load_expected_svg(name, vl_version):
    filename = f"{name}.svg"
    spec_path = specs_dir / "expected" / vl_version / filename
    with spec_path.open("rt", encoding="utf8") as f:
        return f.read()


def load_expected_png(name, vl_version):
    filename = f"{name}.png"
    spec_path = specs_dir / "expected" / vl_version / filename
    with spec_path.open("rb") as f:
        return f.read()


@pytest.mark.parametrize("name", ["circle_binned", "seattle-weather", "stacked_bar_h"])
@pytest.mark.parametrize(
    "vl_version", ["v4_17", "v5_0", "v5_1", "v5_2", "v5_3", "v5_4", "v5_5"]
)
@pytest.mark.parametrize("pretty", [True, False])
def test_reference_specs(name, vl_version, pretty):
    vl_spec = load_vl_spec(name)
    expected_vg_spec = load_expected_vg_spec(name, vl_version, pretty)

    if expected_vg_spec is None:
        with pytest.raises(ValueError):
            vlc.vegalite_to_vega(vl_spec, vl_version=vl_version, pretty=pretty)
    else:
        vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version, pretty=pretty)
        assert expected_vg_spec == vg_spec


@pytest.mark.parametrize("name", ["circle_binned", "stacked_bar_h"])
def test_svg(name):
    vl_version = "v5_5"
    vl_spec = load_vl_spec(name)
    expected_svg = load_expected_svg(name, vl_version)

    # Convert to vega first
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    svg = vlc.vega_to_svg(vg_spec)
    assert svg == expected_svg

    # Convert directly to image
    svg = vlc.vegalite_to_svg(vl_spec, vl_version=vl_version)
    assert svg == expected_svg


@pytest.mark.parametrize("name,scale", [("circle_binned", 1.0), ("stacked_bar_h", 2.0)])
def test_png(name, scale):
    vl_version = "v5_5"
    vl_spec = load_vl_spec(name)
    expected_png = load_expected_png(name, vl_version)

    # Convert to vega first
    vg_spec = vlc.vegalite_to_vega(vl_spec, vl_version=vl_version)
    png = vlc.vega_to_png(vg_spec, scale=scale)
    assert png == expected_png

    # Convert directly to image
    png = vlc.vegalite_to_png(vl_spec, vl_version=vl_version, scale=scale)
    assert png == expected_png
