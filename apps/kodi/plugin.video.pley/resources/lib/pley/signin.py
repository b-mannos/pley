"""The sign-in window: where to go, the code to type there, and the same
as a QR code the server draws, while the plugin asks the server for the
answer."""

from __future__ import annotations

import xbmcgui

CANCEL = 3101
TITLE = 3103
ADDRESS = 3104
CODE = 3105
STATUS = 3106
SCAN = 3107
QR = 3108
# Back and Menu cancel.
DISMISS = (9, 10, 92)


class SignIn(xbmcgui.WindowXMLDialog):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.texts = {}
        self.qr = ""
        self.cancelled = False
        self.ready = False

    def fill(self, title: str, address: str, code: str, status: str, qr: str = "") -> None:
        self.texts = {TITLE: title, ADDRESS: address, CODE: code, STATUS: status}
        self.qr = qr

    def set_status(self, status: str) -> None:
        self.texts[STATUS] = status
        if self.ready:
            self.getControl(STATUS).setLabel(status)

    def onInit(self) -> None:
        self.ready = True
        for control, text in self.texts.items():
            self.getControl(control).setLabel(text)
        self.getControl(SCAN).setVisible(bool(self.qr))
        self.getControl(QR).setVisible(bool(self.qr))
        if self.qr:
            self.getControl(QR).setImage(self.qr, False)
        self.setFocusId(CANCEL)

    def onClick(self, control_id: int) -> None:
        if control_id == CANCEL:
            self.cancel()

    def onAction(self, action) -> None:
        if action.getId() in DISMISS:
            self.cancel()

    def cancel(self) -> None:
        self.cancelled = True
        self.close()
