"""Microsoft Teams plugin for vDeck (vdeck-plugin/1).

Supports two control modes:
1. Primary: Microsoft Teams Local WebSocket API (ws://127.0.0.1:8124) when
   Teams -> Settings -> Privacy -> Manage API (Third-party app API) is enabled.
   Supports mute, camera, raise hand, reactions (like/love/applause/laugh/wow),
   background blur, leave call, and live state sync.
2. Fallback: Targets the Microsoft Teams window directly and sends official
   keyboard shortcuts (Ctrl+Shift+M/O/K/E/H) for core call controls.
"""

from __future__ import annotations

import base64
import ctypes
import ctypes.wintypes as wintypes
import hashlib
import json
import os
import socket
import struct
import sys
import time
import urllib.parse
from pathlib import Path

WS_HOST = "127.0.0.1"
WS_PORT = 8124
WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

VK_CONTROL = 0x11
VK_SHIFT = 0x10
KEYEVENTF_KEYUP = 0x0002
SW_RESTORE = 9

REACTION_LABELS = {
    "like": ("Like", "lucide:thumbs-up", "blue"),
    "love": ("Love", "lucide:heart", "red"),
    "applause": ("Clap", "lucide:sparkles", "amber"),
    "laugh": ("Laugh", "lucide:laugh", "yellow"),
    "wow": ("Wow", "lucide:sparkles", "cyan"),
}

HOTKEYS = {
    "mute": (ord("M"), "Mute Toggled"),
    "camera": (ord("O"), "Camera Toggled"),
    "raise_hand": (ord("K"), "Hand Toggled"),
    "share": (ord("E"), "Share Tray"),
    "leave": (ord("H"), "Left Meeting"),
}

WS_ACTIONS = {
    "mute": ("call", "toggle-mute", None, "Mute Toggled"),
    "camera": ("call", "toggle-video", None, "Camera Toggled"),
    "raise_hand": ("raise-hand", "toggle-hand", None, "Hand Toggled"),
    "blur": ("background-blur", "toggle-background-blur", None, "Blur Toggled"),
    "leave": ("call", "leave-call", None, "Left Meeting"),
}


def send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def token_path() -> Path:
    base = Path(os.environ.get("LOCALAPPDATA") or os.path.expanduser("~"))
    return base / "vdeck" / "teams_token.txt"


def load_token() -> str:
    p = token_path()
    try:
        return p.read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def save_token(token: str) -> None:
    if not token:
        return
    p = token_path()
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(token, encoding="utf-8")
    except OSError:
        pass


class TeamsWsClient:
    """Minimal stdlib RFC 6455 WebSocket client for Teams' local API."""

    def __init__(self) -> None:
        self.sock: socket.socket | None = None
        self.req_id = 1

    def close(self) -> None:
        if self.sock is not None:
            try:
                self.sock.close()
            except OSError:
                pass
            self.sock = None

    def connect(self) -> bool:
        self.close()
        token = load_token()
        query = urllib.parse.urlencode(
            {
                "token": token,
                "protocol-version": "2.0.0",
                "manufacturer": "vDeck",
                "device": "vDeck",
                "app": "vDeck",
                "app-version": "0.1.0",
            }
        )
        try:
            sock = socket.create_connection((WS_HOST, WS_PORT), timeout=1.5)
        except OSError:
            return False

        key_bytes = os.urandom(16)
        sec_key = base64.b64encode(key_bytes).decode("ascii")
        req = (
            f"GET /?{query} HTTP/1.1\r\n"
            f"Host: {WS_HOST}:{WS_PORT}\r\n"
            f"Upgrade: websocket\r\n"
            f"Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {sec_key}\r\n"
            f"Sec-WebSocket-Version: 13\r\n\r\n"
        )
        try:
            sock.sendall(req.encode("ascii"))
            buf = b""
            while b"\r\n\r\n" not in buf:
                chunk = sock.recv(4096)
                if not chunk:
                    sock.close()
                    return False
                buf += chunk
            header = buf.split(b"\r\n\r\n", 1)[0].decode("latin1")
            if "101" not in header.splitlines()[0]:
                sock.close()
                return False
            expected = base64.b64encode(
                hashlib.sha1((sec_key + WS_GUID).encode("ascii")).digest()
            ).decode("ascii")
            if expected.lower() not in header.lower():
                sock.close()
                return False
            self.sock = sock
            return True
        except OSError:
            try:
                sock.close()
            except OSError:
                pass
            return False

    def _send_text(self, text: str) -> None:
        assert self.sock is not None
        payload = text.encode("utf-8")
        mask = os.urandom(4)
        length = len(payload)
        header = bytearray([0x81])
        if length < 126:
            header.append(0x80 | length)
        elif length < 65536:
            header.append(0x80 | 126)
            header.extend(struct.pack("!H", length))
        else:
            header.append(0x80 | 127)
            header.extend(struct.pack("!Q", length))
        header.extend(mask)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(bytes(header) + masked)

    def _recv_exact(self, n: int) -> bytes:
        assert self.sock is not None
        out = bytearray()
        while len(out) < n:
            chunk = self.sock.recv(n - len(out))
            if not chunk:
                raise ConnectionError("WebSocket closed")
            out.extend(chunk)
        return bytes(out)

    def _recv_frame(self) -> tuple[int, bytes]:
        b1, b2 = self._recv_exact(2)
        opcode = b1 & 0x0F
        masked = (b2 & 0x80) != 0
        length = b2 & 0x7F
        if length == 126:
            length = struct.unpack("!H", self._recv_exact(2))[0]
        elif length == 127:
            length = struct.unpack("!Q", self._recv_exact(8))[0]
        mask = self._recv_exact(4) if masked else b""
        payload = self._recv_exact(length)
        if masked:
            payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        return opcode, payload

    def send_action(
        self, service: str, action: str, parameters: dict | None = None
    ) -> dict | None:
        if not self.connect():
            return None
        try:
            self.req_id += 1
            payload: dict = {
                "apiVersion": "2.0.0",
                "service": service,
                "action": action,
                "requestId": self.req_id,
            }
            if parameters:
                payload["parameters"] = parameters
            self._send_text(json.dumps(payload))

            self.sock.settimeout(1.2)  # type: ignore[union-attr]
            deadline = time.monotonic() + 1.2
            latest_msg: dict = {}
            while time.monotonic() < deadline:
                try:
                    opcode, data = self._recv_frame()
                except (TimeoutError, socket.timeout):
                    break
                if opcode == 0x8:
                    break
                if opcode == 0x9:
                    # Pong not strictly needed for short-lived call, close cleanly after.
                    continue
                if opcode != 0x1:
                    continue
                msg = json.loads(data.decode("utf-8"))
                if "tokenRefresh" in msg:
                    save_token(str(msg["tokenRefresh"]))
                latest_msg = msg
                if msg.get("requestId") == self.req_id or "errorMsg" in msg:
                    break
            return latest_msg
        except (OSError, ConnectionError, json.JSONDecodeError):
            return None
        finally:
            self.close()


def find_teams_hwnd() -> int:
    user32 = ctypes.windll.user32
    found: list[int] = []
    EnumWindowsProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def callback(hwnd: int, _lparam: int) -> bool:
        if not user32.IsWindowVisible(hwnd):
            return True
        length = user32.GetWindowTextLengthW(hwnd)
        if length <= 0:
            return True
        buf = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buf, length + 1)
        title = buf.value
        if "Microsoft Teams" in title:
            found.append(hwnd)
            return False
        return True

    user32.EnumWindows(EnumWindowsProc(callback), 0)
    return found[0] if found else 0


def send_teams_hotkey(vk: int) -> bool:
    hwnd = find_teams_hwnd()
    if not hwnd:
        return False
    user32 = ctypes.windll.user32
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, SW_RESTORE)
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.06)
    for k in (VK_CONTROL, VK_SHIFT, vk):
        user32.keybd_event(k, 0, 0, 0)
    for k in (vk, VK_SHIFT, VK_CONTROL):
        user32.keybd_event(k, 0, KEYEVENTF_KEYUP, 0)
    return True


def sync_indicator(
    ind_id: str, show: bool, label: str, icon: str, accent: str
) -> None:
    send(
        {
            "type": "status_indicator",
            "id": ind_id,
            "show": show,
            "label": label,
            "icon": icon,
            "accent": accent,
        }
    )


def sync_teams_indicators(command: str, active: bool) -> None:
    if command == "mute":
        sync_indicator("teams_mute", active, "MUTED", "lucide:mic-off", "red")
    elif command == "raise_hand":
        sync_indicator("teams_hand", active, "HAND", "lucide:hand", "amber")


def sync_meeting_state(ws_resp: dict) -> dict | None:
    update = ws_resp.get("meetingUpdate")
    if not isinstance(update, dict):
        return None
    state = update.get("meetingState")
    if not isinstance(state, dict):
        return None

    # Sync Meeting -> Teams (deck "meeting", layer 0) keys 0=Mute, 1=Camera, 2=Raise Hand.
    if state.get("isInMeeting") is False:
        sync_indicator("teams_mute", False, "MUTED", "lucide:mic-off", "red")
        sync_indicator("teams_hand", False, "HAND", "lucide:hand", "amber")
    if "isMuted" in state:
        muted = bool(state["isMuted"])
        send(
            {
                "type": "event",
                "deck": "meeting",
                "layer": 0,
                "key": 0,
                "active": muted,
            }
        )
        if state.get("isInMeeting") is not False:
            sync_indicator("teams_mute", muted, "MUTED", "lucide:mic-off", "red")
    if "isCameraOn" in state:
        send(
            {
                "type": "event",
                "deck": "meeting",
                "layer": 0,
                "key": 1,
                "active": bool(state["isCameraOn"]),
            }
        )
    if "isHandRaised" in state:
        raised = bool(state["isHandRaised"])
        send(
            {
                "type": "event",
                "deck": "meeting",
                "layer": 0,
                "key": 2,
                "active": raised,
            }
        )
        if state.get("isInMeeting") is not False:
            sync_indicator("teams_hand", raised, "HAND", "lucide:hand", "amber")
    return state


def handle_command(ws: TeamsWsClient, command: str, active: bool, params: dict) -> dict:
    if command == "react":
        reaction = str(params.get("reaction", "like")).lower()
        label, icon, accent = REACTION_LABELS.get(
            reaction, ("Reaction", "lucide:thumbs-up", "blue")
        )
        resp = ws.send_action("call", "send-reaction", {"type": reaction})
        if resp is None:
            return {
                "ok": False,
                "error": "Enable Teams Privacy > Manage API",
            }
        if err := resp.get("errorMsg"):
            return {"ok": False, "error": str(err)}
        send(
            {
                "type": "toast",
                "title": f"Teams: {label}",
                "detail": "Reaction sent",
                "icon": icon,
                "accent": accent,
            }
        )
        return {"ok": True, "status": f"Sent {label}"}

    if command in WS_ACTIONS:
        service, action, ws_params, status_text = WS_ACTIONS[command]
        resp = ws.send_action(service, action, ws_params)
        if resp is not None:
            if err := resp.get("errorMsg"):
                return {"ok": False, "error": str(err)}
            state = sync_meeting_state(resp) or {}
            next_active = None
            if command == "mute" and "isMuted" in state:
                next_active = bool(state["isMuted"])
            elif command == "camera" and "isCameraOn" in state:
                next_active = bool(state["isCameraOn"])
            elif command == "raise_hand" and "isHandRaised" in state:
                next_active = bool(state["isHandRaised"])
            elif command in ("mute", "camera", "raise_hand", "blur"):
                next_active = not active
            out: dict = {"ok": True, "status": status_text}
            if next_active is not None:
                out["active"] = next_active
                sync_teams_indicators(command, next_active)
            return out

    if command in HOTKEYS:
        vk, status_text = HOTKEYS[command]
        if not send_teams_hotkey(vk):
            return {"ok": False, "error": "Microsoft Teams window not found"}
        out = {"ok": True, "status": status_text}
        if command in ("mute", "camera", "raise_hand"):
            next_active = not active
            out["active"] = next_active
            sync_teams_indicators(command, next_active)
        return out

    return {"ok": False, "error": f"unknown command: {command}"}


def main() -> None:
    ws = TeamsWsClient()
    commands = ["mute", "camera", "raise_hand", "react", "blur", "share", "leave"]
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        msg = json.loads(line)
        kind = msg.get("type")

        if kind == "hello":
            send(
                {
                    "type": "hello",
                    "protocol": "vdeck-plugin/1",
                    "name": "teams",
                    "commands": commands,
                }
            )
        elif kind == "call":
            call_id = msg["id"]
            command = msg.get("command", "")
            active = bool(msg.get("active", False))
            params = msg.get("params") or {}
            try:
                res = handle_command(ws, command, active, params)
            except Exception as exc:  # noqa: BLE001
                res = {"ok": False, "error": str(exc)}
            send({"type": "result", "id": call_id, **res})
        elif kind == "goodbye":
            break


if __name__ == "__main__":
    main()
