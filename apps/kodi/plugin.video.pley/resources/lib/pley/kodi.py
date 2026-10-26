"""The Kodi calls pley makes, in one place: settings, strings, the shared
properties the plugin and the service talk through, and what this Kodi is."""

from __future__ import annotations

import json
import os

import xbmc
import xbmcaddon
import xbmcgui
import xbmcvfs

ADDON_ID = "plugin.video.pley"
# Window 10000 (Home) lives as long as Kodi: properties set there are how the
# service tells the plugin where its proxy is, and the plugin tells the
# service what it started playing.
HOME = 10000
PROXY = "pley.proxy"
PLAYING = "pley.playing"

# strings.po ids, by name.
TEXT = {
    "choose_server": 30000,
    "sign_in_to": 30001,
    "sign_out": 30002,
    "search": 30003,
    "playlists": 30004,
    "collections": 30005,
    "next_page": 30006,
    "enter_address": 30007,
    "looking": 30009,
    "sign_in": 30010,
    "sign_in_steps": 30011,
    "signed_in_as": 30012,
    "signed_out": 30013,
    "code_expired": 30014,
    "code_refused": 30015,
    "not_reachable": 30016,
    "not_pley": 30017,
    "service_off": 30018,
    "mark_watched": 30019,
    "mark_unwatched": 30020,
    "add_to_list": 30021,
    "remove_from_list": 30022,
    "stream_stopped": 30024,
    "server_error": 30026,
    "settings": 30027,
    "nothing_found": 30030,
}


def addon() -> xbmcaddon.Addon:
    return xbmcaddon.Addon(ADDON_ID)


def text(name: str) -> str:
    return addon().getLocalizedString(TEXT[name])


def setting(name: str) -> str:
    return addon().getSettingString(name)


def setting_int(name: str) -> int:
    return addon().getSettingInt(name)


def set_setting(name: str, value: str) -> None:
    addon().setSettingString(name, value)


def profile_dir() -> str:
    path = xbmcvfs.translatePath(addon().getAddonInfo("profile"))
    os.makedirs(path, exist_ok=True)
    return path


def addon_dir() -> str:
    return xbmcvfs.translatePath(addon().getAddonInfo("path"))


def log(message: str, level: int = xbmc.LOGINFO) -> None:
    xbmc.log("[pley] " + message, level)


def notify(message: str, error: bool = False) -> None:
    icon = xbmcgui.NOTIFICATION_ERROR if error else xbmcgui.NOTIFICATION_INFO
    xbmcgui.Dialog().notification("pley", message, icon, 5000)


def get_property(name: str) -> str:
    return xbmcgui.Window(HOME).getProperty(name)


def set_property(name: str, value: str) -> None:
    xbmcgui.Window(HOME).setProperty(name, value)


def clear_property(name: str) -> None:
    xbmcgui.Window(HOME).clearProperty(name)


def kodi_version() -> str:
    """"21.3" of "21.3 (21.3.0) Git:…"."""
    return xbmc.getInfoLabel("System.BuildVersion").split(" ")[0] or "unknown"


def platform() -> str:
    for condition, name in (
        ("System.Platform.Android", "Android"),
        ("System.Platform.Windows", "Windows"),
        ("System.Platform.OSX", "macOS"),
        ("System.Platform.IOS", "iOS"),
        ("System.Platform.TVOS", "tvOS"),
        ("System.Platform.Linux", "Linux"),
    ):
        if xbmc.getCondVisibility(condition):
            return name
    return "unknown"


def is_coreelec() -> bool:
    try:
        with open("/etc/os-release", encoding="utf-8") as release:
            return "coreelec" in release.read().lower()
    except OSError:
        return False


def user_agent() -> str:
    version = addon().getAddonInfo("version")
    return "pleyKodi/{} (Kodi {}; {})".format(version, kodi_version(), platform())


def setting_value(name: str):
    """One of Kodi's own settings, through JSON-RPC (None when it has no
    such setting)."""
    request = {"jsonrpc": "2.0", "id": 1, "method": "Settings.GetSettingValue", "params": {"setting": name}}
    try:
        answer = json.loads(xbmc.executeJSONRPC(json.dumps(request)))
    except ValueError:
        return None
    return answer.get("result", {}).get("value")
