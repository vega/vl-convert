import vl_convert as vlc


def test_get_themes_dark_theme_background():
    themes = vlc.get_themes()
    assert isinstance(themes, dict)
    dark = themes["dark"]
    assert isinstance(dark, dict)
    background = dark["background"]
    assert background == "#333"


def test_cargong10_theme():
    themes = vlc.get_themes()
    assert themes["carbong10"] == {
        "arc": {"fill": "#6929c4"},
        "area": {"fill": "#6929c4"},
        "axis": {
            "grid": True,
            "gridColor": "#e0e0e0",
            "labelAngle": 0,
            "labelColor": "#525252",
            "labelFont": "IBM Plex Sans Condensed, system-ui, -apple-system, "
            'BlinkMacSystemFont, ".SFNSText-Regular", sans-serif',
            "labelFontSize": 12,
            "labelFontWeight": 400,
            "titleColor": "#161616",
            "titleFontSize": 12,
            "titleFontWeight": 600,
        },
        "axisX": {"titlePadding": 10},
        "axisY": {"titlePadding": 2.5},
        "background": "#f4f4f4",
        "circle": {"fill": "#6929c4"},
        "group": {"fill": "#ffffff"},
        "path": {"stroke": "#6929c4"},
        "range": {
            "category": [
                "#6929c4",
                "#1192e8",
                "#005d5d",
                "#9f1853",
                "#fa4d56",
                "#570408",
                "#198038",
                "#002d9c",
                "#ee538b",
                "#b28600",
                "#009d9a",
                "#012749",
                "#8a3800",
                "#a56eff",
            ],
            "diverging": [
                "#750e13",
                "#a2191f",
                "#da1e28",
                "#fa4d56",
                "#ff8389",
                "#ffb3b8",
                "#ffd7d9",
                "#fff1f1",
                "#e5f6ff",
                "#bae6ff",
                "#82cfff",
                "#33b1ff",
                "#1192e8",
                "#0072c3",
                "#00539a",
                "#003a6d",
            ],
            "heatmap": [
                "#f6f2ff",
                "#e8daff",
                "#d4bbff",
                "#be95ff",
                "#a56eff",
                "#8a3ffc",
                "#6929c4",
                "#491d8b",
                "#31135e",
                "#1c0f30",
            ],
        },
        "rect": {"fill": "#6929c4"},
        "shape": {"stroke": "#6929c4"},
        "style": {
            "guide-label": {
                "fill": "#525252",
                "font": "IBM Plex "
                'Sans,system-ui,-apple-system,BlinkMacSystemFont,".sfnstext-regular",sans-serif',
                "fontWeight": 400,
            },
            "guide-title": {
                "fill": "#525252",
                "font": "IBM Plex "
                'Sans,system-ui,-apple-system,BlinkMacSystemFont,".sfnstext-regular",sans-serif',
                "fontWeight": 400,
            },
        },
        "symbol": {"stroke": "#6929c4"},
        "title": {
            "anchor": "start",
            "color": "#161616",
            "dy": -15,
            "font": "IBM Plex "
            'Sans,system-ui,-apple-system,BlinkMacSystemFont,".sfnstext-regular",sans-serif',
            "fontSize": 16,
            "fontWeight": 600,
        },
        "view": {"fill": "#ffffff", "stroke": "#ffffff"},
    }
