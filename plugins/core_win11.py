"""Core Windows 11 native plugin for vDeck (vdeck-plugin/1).

Uses native Win11 URI schemes (Windows Clock, Snipping Tool, Sticky Notes)
and Win32 user32 APIs (LockWorkStation, media/volume/mic keys).
"""

import ctypes
import json
import os
import shutil
import subprocess
import sys
import threading
import urllib.parse
import winreg
from pathlib import Path

PLUGIN_CONFIG: dict = {}

VK_VOLUME_MUTE = 0xAD
VK_VOLUME_DOWN = 0xAE
VK_VOLUME_UP = 0xAF
VK_MEDIA_PLAY_PAUSE = 0xB3
VK_SNAPSHOT = 0x2C
VK_MENU = 0x12
KEYEVENTF_KEYUP = 0x0002

WM_APPCOMMAND = 0x0319
APPCOMMAND_MICROPHONE_VOLUME_MUTE = 24 << 16

PUSH_NOTIFICATIONS_KEY = r"Software\Microsoft\Windows\CurrentVersion\PushNotifications"


def get_toast_enabled() -> bool:
    """Returns True if Windows PushNotifications ToastEnabled != 0 (DND off)."""
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, PUSH_NOTIFICATIONS_KEY, 0, winreg.KEY_READ) as key:
            val, _ = winreg.QueryValueEx(key, "ToastEnabled")
            return int(val) != 0
    except OSError:
        return True


def set_toast_enabled(enabled: bool) -> None:
    """Sets HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\PushNotifications\\ToastEnabled."""
    with winreg.CreateKeyEx(
        winreg.HKEY_CURRENT_USER, PUSH_NOTIFICATIONS_KEY, 0, winreg.KEY_WRITE
    ) as key:
        winreg.SetValueEx(key, "ToastEnabled", 0, winreg.REG_DWORD, 1 if enabled else 0)


def sync_indicator(
    ind_id: str, show: bool, label: str, icon: str, accent: str
) -> None:
    send({
        "type": "status_indicator",
        "id": ind_id,
        "show": show,
        "label": label,
        "icon": icon,
        "accent": accent,
    })


def sync_dnd_indicator(dnd_active: bool) -> None:
    sync_indicator("dnd", dnd_active, "DND", "lucide:bell-off", "amber")


def send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def tap_vk(vk: int) -> None:
    user32 = ctypes.windll.user32
    user32.keybd_event(vk, 0, 0, 0)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)


def open_uri_or_fallback(uri: str, fallback: list[str] | None = None) -> None:
    try:
        os.startfile(uri)  # type: ignore[attr-defined]
    except OSError:
        if fallback:
            subprocess.Popen(fallback, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def tap_combo(*vks: int) -> None:
    user32 = ctypes.windll.user32
    for vk in vks:
        user32.keybd_event(vk, 0, 0, 0)
    for vk in reversed(vks):
        user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)


def find_sharex() -> str | None:
    configured = str(PLUGIN_CONFIG.get("sharex_path", "")).strip()
    if configured and Path(configured).exists():
        return configured
    if found := shutil.which("ShareX"):
        return found
    candidates = [
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "ShareX" / "ShareX.exe",
        Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "ShareX" / "ShareX.exe",
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "ShareX" / "ShareX.exe",
        Path(os.path.expanduser("~")) / "scoop" / "apps" / "sharex" / "current" / "ShareX.exe",
    ]
    for c in candidates:
        if c.exists():
            return str(c)
    return None


def launch_notes() -> str:
    app = str(PLUGIN_CONFIG.get("notes_app", "obsidian")).strip()
    lower = app.lower()
    if lower == "obsidian":
        if custom_uri := str(PLUGIN_CONFIG.get("obsidian_uri", "")).strip():
            os.startfile(custom_uri)  # type: ignore[attr-defined]
            return "Obsidian"
        vault = str(PLUGIN_CONFIG.get("obsidian_vault", "")).strip()
        uri = (
            f"obsidian://daily?vault={urllib.parse.quote(vault)}"
            if vault
            else "obsidian://daily"
        )
        open_uri_or_fallback(uri, ["notepad.exe"])
        return "Obsidian Daily"
    if lower in ("sticky_notes", "stickynotes", "sticky"):
        open_uri_or_fallback("ms-stickynotes:", ["notepad.exe"])
        return "Sticky Notes"
    if lower == "notepad":
        subprocess.Popen(["notepad.exe"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return "Notepad"
    if "://" in app:
        open_uri_or_fallback(app, ["notepad.exe"])
        return "Notes"
    subprocess.Popen([app], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return Path(app).stem or "Notes"


def build_capture_deck() -> dict:
    empty = {"label": "", "icon": "circle", "accent": "cyan", "action": {"type": "none"}}
    keys = [
        {
            "label": "Screen",
            "icon": "lucide:monitor",
            "accent": "cyan",
            "action": {"type": "plugin", "plugin": "win11", "command": "capture_screen"},
        },
        {
            "label": "Window",
            "icon": "lucide:app-window",
            "accent": "blue",
            "action": {"type": "plugin", "plugin": "win11", "command": "capture_window"},
        },
        {
            "label": "Selection",
            "icon": "lucide:crop",
            "accent": "green",
            "action": {"type": "plugin", "plugin": "win11", "command": "capture_selection"},
        },
        {
            "label": "GIF",
            "icon": "lucide:film",
            "accent": "amber",
            "action": {"type": "plugin", "plugin": "win11", "command": "capture_gif"},
        },
        {
            "label": "Record",
            "icon": "lucide:video",
            "accent": "red",
            "action": {"type": "plugin", "plugin": "win11", "command": "capture_video"},
        },
        empty,
        {"label": "Back", "icon": "back", "accent": "orange", "action": {"type": "back"}},
        empty,
        empty,
    ]
    return {"id": "capture", "name": "Capture", "layers": [{"name": "ShareX", "keys": keys}]}


def run_capture(mode: str) -> tuple[bool, str]:
    app = str(PLUGIN_CONFIG.get("capture_app", "sharex")).strip().lower()
    # Delay capture until after vDeck's 260ms close/hide animation finishes so
    # ShareX -ActiveWindow (and fullscreen/region captures) don't capture vDeck.
    delay_s = float(PLUGIN_CONFIG.get("capture_delay_s", 0.40))

    if app == "sharex":
        sharex = find_sharex()
        if sharex:
            flag_map = {
                "screen": "-PrintScreen",
                "window": "-ActiveWindow",
                "selection": "-RectangleRegion",
                "gif": "-ScreenRecorderGIF",
                "video": "-ScreenRecorder",
            }
            flag = flag_map.get(mode, "-RectangleRegion")
            threading.Timer(
                delay_s,
                lambda: subprocess.Popen(
                    [sharex, flag], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
                ),
            ).start()
            return True, f"ShareX {mode.title()}"
        if mode in ("gif", "video"):
            return False, "ShareX.exe not found"

    # Snipping Tool / native Windows fallback
    if mode == "screen":
        threading.Timer(delay_s, lambda: tap_vk(VK_SNAPSHOT)).start()
        return True, "Screen Captured"
    if mode == "window":
        threading.Timer(delay_s, lambda: tap_combo(VK_MENU, VK_SNAPSHOT)).start()
        return True, "Window Captured"
    if mode == "selection":
        threading.Timer(delay_s, lambda: open_uri_or_fallback("ms-screenclip:")).start()
        return True, "Snipping Tool"
    return False, f"{mode} requires ShareX"


def handle_command(command: str, active: bool) -> dict:
    if command == "lock":
        ctypes.windll.user32.LockWorkStation()
        return {"ok": True, "status": "Locked"}

    if command == "timer":
        open_uri_or_fallback("ms-clock:")
        return {"ok": True, "active": not active, "status": "Windows Clock"}

    if command in ("focus", "dnd"):
        currently_dnd = not get_toast_enabled()
        next_dnd = not currently_dnd
        set_toast_enabled(not next_dnd)
        sync_dnd_indicator(next_dnd)
        status_text = "DND On" if next_dnd else "DND Off"
        send({
            "type": "toast",
            "title": "Do Not Disturb",
            "detail": status_text,
            "icon": "lucide:bell-off" if next_dnd else "sun",
            "accent": "amber",
        })
        return {"ok": True, "active": next_dnd, "status": status_text}

    if command == "notes":
        status = launch_notes()
        return {"ok": True, "status": status}

    if command == "capture":
        return {
            "ok": True,
            "decks": [build_capture_deck()],
            "open_deck": "capture",
            "status": "Capture",
        }

    if command in (
        "capture_screen",
        "capture_window",
        "capture_selection",
        "capture_gif",
        "capture_video",
    ):
        mode = command.removeprefix("capture_")
        ok, status = run_capture(mode)
        if not ok:
            return {"ok": False, "error": status}
        return {"ok": True, "status": status}

    if command == "snippet":
        open_uri_or_fallback("ms-screenclip:")
        return {"ok": True, "status": "Snipping Tool"}

    if command == "terminal":
        configured = str(PLUGIN_CONFIG.get("terminal_app", "")).strip()
        exe = configured or shutil.which("wt") or shutil.which("pwsh") or "powershell.exe"
        subprocess.Popen([exe], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return {"ok": True, "status": "Terminal"}

    if command == "agent":
        agent_cmd = str(PLUGIN_CONFIG.get("agent_cmd", "opencode")).strip() or "opencode"
        wt = shutil.which("wt")
        if wt:
            subprocess.Popen(
                [wt, "new-tab", "pwsh", "-NoExit", "-Command", agent_cmd],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        else:
            shell = shutil.which("pwsh") or "powershell.exe"
            subprocess.Popen(
                [shell, "-NoExit", "-Command", agent_cmd],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        return {"ok": True, "status": agent_cmd}

    if command == "browser":
        open_uri_or_fallback("https://")
        return {"ok": True, "status": "Browser"}

    if command == "mute":
        tap_vk(VK_VOLUME_MUTE)
        next_active = not active
        sync_indicator("sys_mute", next_active, "SYS MUTE", "lucide:volume-x", "red")
        return {
            "ok": True,
            "active": next_active,
            "status": "Audio Muted" if next_active else "Audio Unmuted",
        }

    if command == "mic_mute":
        user32 = ctypes.windll.user32
        # Target a single shell/foreground window; HWND_BROADCAST triggers WM_APPCOMMAND
        # across every top-level window and causes repeated mute/unmute toggling.
        hwnd = user32.GetShellWindow() or user32.GetForegroundWindow()
        if hwnd:
            user32.PostMessageW(hwnd, WM_APPCOMMAND, hwnd, APPCOMMAND_MICROPHONE_VOLUME_MUTE)
        next_active = not active
        sync_indicator("mic_mute", next_active, "MIC MUTE", "lucide:mic-off", "red")
        return {"ok": True, "active": next_active, "status": "Mic Toggled"}

    if command == "play_pause":
        tap_vk(VK_MEDIA_PLAY_PAUSE)
        return {"ok": True, "active": not active, "status": "Play/Pause"}

    if command == "vol_down":
        tap_vk(VK_VOLUME_DOWN)
        return {"ok": True, "status": "Vol -"}

    if command == "vol_up":
        tap_vk(VK_VOLUME_UP)
        return {"ok": True, "status": "Vol +"}

    return {"ok": False, "error": f"unknown command: {command}"}


def main() -> None:
    commands = [
        "lock",
        "timer",
        "focus",
        "dnd",
        "notes",
        "snippet",
        "capture",
        "capture_screen",
        "capture_window",
        "capture_selection",
        "capture_gif",
        "capture_video",
        "terminal",
        "agent",
        "browser",
        "mute",
        "mic_mute",
        "play_pause",
        "vol_down",
        "vol_up",
    ]
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        msg = json.loads(line)
        kind = msg.get("type")

        if kind == "hello":
            PLUGIN_CONFIG.clear()
            PLUGIN_CONFIG.update(msg.get("plugin_config") or {})
            send({
                "type": "hello",
                "protocol": "vdeck-plugin/1",
                "name": "win11",
                "commands": commands,
            })
            if not get_toast_enabled():
                sync_dnd_indicator(True)
                app_cfg = msg.get("config") or {}
                for deck in app_cfg.get("decks") or []:
                    deck_id = deck.get("id")
                    for layer_idx, layer in enumerate(deck.get("layers") or []):
                        for key_idx, key in enumerate(layer.get("keys") or []):
                            action = key.get("action") or {}
                            if (
                                action.get("type") == "plugin"
                                and action.get("plugin") == "win11"
                                and action.get("command") in ("focus", "dnd")
                            ):
                                send({
                                    "type": "event",
                                    "deck": deck_id,
                                    "layer": layer_idx,
                                    "key": key_idx,
                                    "active": True,
                                })
        elif kind == "call":
            call_id = msg["id"]
            command = msg.get("command", "")
            active = bool(msg.get("active", False))
            try:
                res = handle_command(command, active)
            except Exception as exc:  # noqa: BLE001
                res = {"ok": False, "error": str(exc)}
            send({"type": "result", "id": call_id, **res})
        elif kind == "goodbye":
            break


if __name__ == "__main__":
    main()
