import json

import vl_convert as vlc

themes = vlc.get_themes()
print(json.dumps(sorted(themes), indent=2), end="\n\n")
print(json.dumps(themes["dark"], indent=2))
