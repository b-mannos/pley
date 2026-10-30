"""Stand-ins for Kodi's modules, enough for the add-on's tests."""

LOGDEBUG, LOGINFO, LOGWARNING, LOGERROR = 0, 1, 2, 3
info_labels = {}
conditions = set()
builtins = []
logged = []


def log(message, level=LOGINFO):
    logged.append((level, message))


# Kodi's regional formats (getRegion).
region = {"time": "%H:%M:%S"}


def getRegion(name):
    return region.get(name, "")


def getInfoLabel(name):
    return info_labels.get(name, "")


def getCondVisibility(name):
    return name in conditions


def sleep(ms):
    pass


def executebuiltin(command, wait=False):
    builtins.append(command)


def executeJSONRPC(request):
    return '{"result": {"value": null}}'


PLAYLIST_MUSIC, PLAYLIST_VIDEO = 0, 1
# What Player.play was given: (item, listitem, startpos).
played = []


class Player:
    def __init__(self):
        pass

    def play(self, item=None, listitem=None, windowed=False, startpos=-1):
        played.append((item, listitem, startpos))


class PlayList:
    """Kodi's playlists, one of each kind: what is in them, and whether
    they are shuffled."""

    entries = {}
    shuffled = {}

    def __init__(self, kind):
        self.kind = kind

    def clear(self):
        PlayList.entries[self.kind] = []

    def add(self, url, listitem=None, index=-1):
        PlayList.entries.setdefault(self.kind, []).append((url, listitem))

    def shuffle(self):
        PlayList.shuffled[self.kind] = True

    def unshuffle(self):
        PlayList.shuffled[self.kind] = False

    def size(self):
        return len(PlayList.entries.get(self.kind, []))

    def __getitem__(self, at):
        import xbmcgui

        url, _ = PlayList.entries.get(self.kind, [])[at]
        return xbmcgui.ListItem(path=url)


class Monitor:
    def waitForAbort(self, seconds=0):
        return True


class Actor:
    def __init__(self, name="", role="", order=-1, thumbnail=""):
        self.name, self.role, self.order, self.thumbnail = name, role, order, thumbnail
