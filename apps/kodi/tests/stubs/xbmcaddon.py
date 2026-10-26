settings = {}


class Addon:
    def __init__(self, id=None):
        pass

    def getSettingString(self, name):
        return str(settings.get(name, ""))

    def getSettingInt(self, name):
        return int(settings.get(name, 0))

    def setSettingString(self, name, value):
        settings[name] = value

    def getLocalizedString(self, id):
        return "#%d %%s" % id

    def getAddonInfo(self, name):
        return {"version": "0.1.0", "profile": settings.get("_profile", "."), "path": "."}[name]
