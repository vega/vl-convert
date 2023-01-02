import vl_convert as vlc


def test_get_local_tz_is_str_or_none():
    # Just check that get_local_tz runs and returns a string or None
    local_tz = vlc.get_local_tz()
    assert isinstance(local_tz, str) or local_tz is None
