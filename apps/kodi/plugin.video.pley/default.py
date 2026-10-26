import os
import sys

# Kodi keeps this Python between calls (reuselanguageinvoker): the path is
# added once.
LIB = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resources", "lib")
if LIB not in sys.path:
    sys.path.insert(0, LIB)

from pley import plugin  # noqa: E402

plugin.run(sys.argv)
