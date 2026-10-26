"""Packs the add-on as Kodi installs it from a zip file:
dist/plugin.video.pley-<version>.zip, with the add-on's folder at its root.

    python apps/kodi/package.py
"""

import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ADDON = "plugin.video.pley"
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))


def version() -> str:
    with open(os.path.join(HERE, ADDON, "addon.xml"), encoding="utf-8") as f:
        return re.search(r'<addon id="%s"[^>]* version="([^"]+)"' % re.escape(ADDON), f.read()).group(1)


def main() -> int:
    out = os.path.join(ROOT, "dist", "%s-%s.zip" % (ADDON, version()))
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as archive:
        for folder, dirs, files in os.walk(os.path.join(HERE, ADDON)):
            dirs[:] = sorted(d for d in dirs if d != "__pycache__")
            for name in sorted(files):
                if name.endswith((".pyc", ".pyo")):
                    continue
                path = os.path.join(folder, name)
                archive.write(path, os.path.relpath(path, HERE).replace(os.sep, "/"))
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
