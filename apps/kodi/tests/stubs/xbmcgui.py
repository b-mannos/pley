NOTIFICATION_INFO, NOTIFICATION_WARNING, NOTIFICATION_ERROR = "info", "warning", "error"
current_window = 10000
# Notifications shown: (heading, message); boxes shown: (heading, message).
notified = []
boxes = []


def getCurrentWindowId():
    return current_window


class Tag:
    """Records what each setter was given: tag.values["setTitle"]."""

    def __init__(self):
        self.values = {}

    def __getattr__(self, name):
        if not name.startswith("set"):
            raise AttributeError(name)

        def setter(*args):
            self.values[name] = args[0] if len(args) == 1 else args

        return setter


class ListItem:
    def __init__(self, label="", label2="", path="", offscreen=False):
        self.label = label
        self.label2 = label2
        self.selected = False
        self.path = path
        self.art = {}
        self.properties = {}
        self.menu = []
        self.video = Tag()
        self.music = Tag()
        self.subtitles = []
        self.mime = None

    def getPath(self):
        return self.path

    def setLabel(self, label):
        self.label = label

    def setLabel2(self, label):
        self.label2 = label

    def setArt(self, art):
        self.art.update(art)

    def setProperty(self, key, value):
        self.properties[key] = value

    def getVideoInfoTag(self):
        return self.video

    def getMusicInfoTag(self):
        return self.music

    def addContextMenuItems(self, items):
        self.menu += items

    def setPath(self, path):
        self.path = path

    def setMimeType(self, mime):
        self.mime = mime

    def setContentLookup(self, enabled):
        self.lookup = enabled

    def setSubtitles(self, subtitles):
        self.subtitles = subtitles

    def select(self, selected):
        self.selected = selected


class Window:
    properties = {}

    def __init__(self, id=0):
        pass

    def getProperty(self, key):
        return Window.properties.get(key, "")

    def setProperty(self, key, value):
        Window.properties[key] = value

    def clearProperty(self, key):
        Window.properties.pop(key, None)


class WindowXML:
    def __init__(self, *args, **kwargs):
        pass

    def close(self):
        pass


class WindowXMLDialog:
    def __init__(self, *args, **kwargs):
        pass

    def show(self):
        pass

    def close(self):
        pass


class Dialog:
    def notification(self, heading, message, icon="", time=0, sound=True):
        notified.append((heading, message))

    def ok(self, heading, message):
        boxes.append((heading, message))
        return True
