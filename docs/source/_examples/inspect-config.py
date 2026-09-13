import json

import vl_convert as vlc

vlc.load_config("production.vlc.jsonc")
vlc.configure(num_workers=2)
print(json.dumps(vlc.get_config(), indent=2))
