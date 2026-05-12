---
title: Python Configuration
path: advanced/python-configuration
section: Advanced
order: 410
interfaces: [python]
---

<!-- topic-body -->

# Python Configuration

`configure()` patches Python process state. `load_config()` replaces that
state from a JSONC config file.

```python
import vl_convert as vlc

vlc.configure(num_workers=4, auto_google_fonts=True)
cfg = vlc.get_config()
```

Use `warm_up_workers()` after changing worker settings when latency on the
first request matters.
