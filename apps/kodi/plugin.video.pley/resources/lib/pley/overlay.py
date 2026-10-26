"""The button over the video: Skip Intro, Skip Credits, or Up Next with its
countdown. It shows without pausing playback; what was pressed is read by
the service's loop."""

from __future__ import annotations

import xbmcgui

ACCEPT = 3001
CANCEL = 3002
TITLE = 3003
# Back and Menu dismiss it.
DISMISS = (9, 10, 92)


class Overlay(xbmcgui.WindowXMLDialog):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.title = ""
        self.accept = ""
        self.cancel = ""
        self.choice = None
        self.ready = False
        # Told of a choice at once, so a skip doesn't wait for the
        # service's next turn.
        self.on_choice = None

    def set_texts(self, title: str, accept: str, cancel: str = "") -> None:
        self.title, self.accept, self.cancel = title, accept, cancel
        if self.ready:
            self.apply()

    def apply(self) -> None:
        self.getControl(TITLE).setLabel(self.title)
        self.getControl(TITLE).setVisible(bool(self.title))
        self.getControl(ACCEPT).setLabel(self.accept)
        self.getControl(CANCEL).setLabel(self.cancel)
        self.getControl(CANCEL).setVisible(bool(self.cancel))

    def onInit(self) -> None:
        self.ready = True
        self.apply()
        self.setFocusId(ACCEPT)

    def onClick(self, control_id: int) -> None:
        self.choose("accept" if control_id == ACCEPT else "cancel")

    def onAction(self, action) -> None:
        if action.getId() in DISMISS:
            self.choose("cancel")

    def choose(self, choice: str) -> None:
        self.choice = choice
        self.close()
        if self.on_choice:
            self.on_choice(self)
