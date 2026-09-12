import json
from pathlib import Path

import vl_convert as vlc

vlc.configure(
    auto_google_fonts=True,
    embed_local_fonts=True,
    missing_fonts="error",
)
spec = Path("font-introspection.vl.json").read_text(encoding="utf-8")
fonts = vlc.vegalite_fonts(spec)
print(json.dumps(fonts, indent=2))
