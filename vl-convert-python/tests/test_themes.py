import vl_convert as vlc


def test_get_themes_dark_theme_background():
    themes = vlc.get_themes()
    assert isinstance(themes, dict)
    dark = themes["dark"]
    assert isinstance(dark, dict)
    background = dark["background"]
    assert background == "#333"
