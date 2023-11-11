import vl_convert as vlc


def test_get_format_locale():
    format_locale = vlc.get_format_locale("it-IT")
    assert format_locale == {
        "currency": ["€", ""],
        "decimal": ",",
        "grouping": [3],
        "thousands": ".",
    }


def test_get_time_format_locale():
    format_locale = vlc.get_time_format_locale("it-IT")
    assert format_locale == {
        "date": "%d/%m/%Y",
        "dateTime": "%A %e %B %Y, %X",
        "days": [
            "Domenica",
            "Lunedì",
            "Martedì",
            "Mercoledì",
            "Giovedì",
            "Venerdì",
            "Sabato",
        ],
        "months": [
            "Gennaio",
            "Febbraio",
            "Marzo",
            "Aprile",
            "Maggio",
            "Giugno",
            "Luglio",
            "Agosto",
            "Settembre",
            "Ottobre",
            "Novembre",
            "Dicembre",
        ],
        "periods": ["AM", "PM"],
        "shortDays": ["Dom", "Lun", "Mar", "Mer", "Gio", "Ven", "Sab"],
        "shortMonths": [
            "Gen",
            "Feb",
            "Mar",
            "Apr",
            "Mag",
            "Giu",
            "Lug",
            "Ago",
            "Set",
            "Ott",
            "Nov",
            "Dic",
        ],
        "time": "%H:%M:%S",
    }
