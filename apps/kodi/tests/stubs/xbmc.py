"""Stand-ins for Kodi's modules, enough for the add-on's tests."""

LOGDEBUG, LOGINFO, LOGWARNING, LOGERROR = 0, 1, 2, 3
info_labels = {}
conditions = set()
builtins = []
logged = []


def log(message, level=LOGINFO):
    logged.append((level, message))


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


class Player:
    def __init__(self):
        pass


class Monitor:
    def waitForAbort(self, seconds=0):
        return True


class Actor:
    def __init__(self, name="", role="", order=-1, thumbnail=""):
        self.name, self.role, self.order, self.thumbnail = name, role, order, thumbnail
