import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "resources", "lib"))

from pley import plugin  # noqa: E402

plugin.run(sys.argv)
