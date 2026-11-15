#!/usr/bin/env python3
"""vDeck PowerToys integration plugin (vdeck-plugin/1)."""

import ctypes
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

PROTOCOL = "vdeck-plugin/1"

VK_LWIN = 0x5B
VK_CONTROL = 0x11
VK_MENU = 0x12  # Alt
VK_SHIFT = 0x10
KEYEVENTF_KEYUP = 0x0002

# (settings_folder, shortcut_property_key, default_shortcut, display_label)
UTILITIES: dict[str, tuple[str, str, dict, str]] = {
    "text_extractor": (
        "TextExtractor",
        "ActivationShortcut",
        {"win": True, "ctrl": False, "alt": False, "shift": True, "code": 84},  # Win+Shift+T
        "Text Extract",
    ),
    "color_picker": (
        "ColorPicker",
        "ActivationShortcut",
        {"win": True, "ctrl": False, "alt": False, "shift": True, "code": 67},  # Win+Shift+C
        "Color Picker",
    ),
    "screen_ruler": (
        "Measure Tool",
        "ActivationShortcut",
        {"win": True, "ctrl": False, "alt": False, "shift": True, "code": 77},  # Win+Shift+M
        "Screen Ruler",
    ),
    "fancyzones_editor": (
        "FancyZones",
        "fancyzones_editor_hotkey",
        {"win": True, "ctrl": False, "alt": False, "shift": True, "code": 192},  # Win+Shift+`
        "FancyZones",
    ),
    "always_on_top": (
        "AlwaysOnTop",
        "hotkey",
        {"win": True, "ctrl": True, "alt": False, "shift": False, "code": 84},  # Win+Ctrl+T
        "Always Top",
    ),
    "advanced_paste": (
        "AdvancedPaste",
        "advanced-paste-ui-shortcut",
        {"win": True, "ctrl": False, "alt": False, "shift": True, "code": 86},  # Win+Shift+V
        "Adv Paste",
    ),
    "paste_plain": (
        "PastePlain",
        "activation_shortcut",
        {"win": True, "ctrl": True, "alt": True, "shift": False, "code": 86},  # Win+Ctrl+Alt+V
        "Paste Plain",
    ),
    "powertoys_run": (
        "PowerToys Run",
        "open_powerlauncher",
        {"win": False, "ctrl": False, "alt": True, "shift": False, "code": 32},  # Alt+Space
        "PT Run",
    ),
    "workspaces": (
        "Workspaces",
        "hotkey",
        {"win": True, "ctrl": True, "alt": False, "shift": False, "code": 192},  # Win+Ctrl+`
        "Workspaces",
    ),
}

COMMANDS = [*UTILITIES.keys(), "settings", "menu"]


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def read_shortcut(folder: str, prop_key: str, default: dict) -> dict:
    local_app = os.environ.get("LOCALAPPDATA", "")
    if not local_app:
        return default
    settings_path = Path(local_app) / "Microsoft" / "PowerToys" / folder / "settings.json"
    if not settings_path.is_file():
        return default
    try:
        data = json.loads(settings_path.read_text(encoding="utf-8"))
        props = data.get("properties") or {}
        entry = props.get(prop_key)
        if isinstance(entry, dict) and "value" in entry and isinstance(entry["value"], dict):
            entry = entry["value"]
        if isinstance(entry, dict) and int(entry.get("code", 0)) > 0:
            return entry
    except Exception:
        pass
    return default


def send_shortcut(shortcut: dict) -> None:
    if sys.platform != "win32":
        return
    user32 = ctypes.windll.user32
    mods: list[int] = []
    if shortcut.get("win"):
        mods.append(VK_LWIN)
    if shortcut.get("ctrl"):
        mods.append(VK_CONTROL)
    if shortcut.get("alt"):
        mods.append(VK_MENU)
    if shortcut.get("shift"):
        mods.append(VK_SHIFT)

    vk = int(shortcut.get("code", 0))
    if vk <= 0:
        raise ValueError("invalid shortcut key code")

    for m in mods:
        user32.keybd_event(m, 0, 0, 0)
    user32.keybd_event(vk, 0, 0, 0)
    time.sleep(0.02)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)
    for m in reversed(mods):
        user32.keybd_event(m, 0, KEYEVENTF_KEYUP, 0)


def open_powertoys_settings() -> None:
    local_app = os.environ.get("LOCALAPPDATA", "")
    exe = Path(local_app) / "PowerToys" / "PowerToys.exe"
    if exe.is_file():
        subprocess.Popen([str(exe)])
        return
    pf = os.environ.get("ProgramFiles", r"C:\Program Files")
    exe_pf = Path(pf) / "PowerToys" / "PowerToys.exe"
    if exe_pf.is_file():
        subprocess.Popen([str(exe_pf)])
        return
    raise FileNotFoundError("PowerToys.exe not found")


def build_menu_deck() -> dict:
    def pt_key(label: str, icon: str, accent: str, command: str) -> dict:
        return {
            "label": label,
            "icon": icon,
            "accent": accent,
            "action": {"type": "plugin", "plugin": "powertoys", "command": command},
        }

    return {
        "id": "powertoys_menu",
        "name": "PowerToys",
        "layers": [
            {
                "name": "Utilities",
                "keys": [
                    pt_key("Text Extract", "scissors", "cyan", "text_extractor"),
                    pt_key("Color Picker", "circle", "purple", "color_picker"),
                    pt_key("Screen Ruler", "layers", "amber", "screen_ruler"),
                    pt_key("FancyZones", "layers", "blue", "fancyzones_editor"),
                    pt_key("Always Top", "arrow_up", "teal", "always_on_top"),
                    pt_key("Adv Paste", "play", "green", "advanced_paste"),
                    {"label": "Back", "icon": "back", "accent": "orange", "action": {"type": "back"}},
                    pt_key("PT Run", "terminal", "blue", "powertoys_run"),
                    pt_key("Workspaces", "folder", "purple", "workspaces"),
                ],
            }
        ],
    }


def handle_call(msg_id: int, command: str) -> dict:
    try:
        if command == "menu":
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [build_menu_deck()],
                "open_deck": "powertoys_menu",
                "status": "PowerToys",
            }
        if command == "settings":
            open_powertoys_settings()
            return {"type": "result", "id": msg_id, "ok": True, "status": "PowerToys"}
        if command in UTILITIES:
            folder, prop_key, default_sc, label = UTILITIES[command]
            shortcut = read_shortcut(folder, prop_key, default_sc)
            # Fire after vDeck's 260ms hide completes so focus returns to the target window.
            timer = threading.Timer(0.40, send_shortcut, args=(shortcut,))
            timer.daemon = True
            timer.start()
            return {"type": "result", "id": msg_id, "ok": True, "status": label}
        return {
            "type": "result",
            "id": msg_id,
            "ok": False,
            "error": f"unknown command: {command}",
        }
    except Exception as exc:
        return {"type": "result", "id": msg_id, "ok": False, "error": str(exc)}


def main() -> None:
    if "--stdio" not in sys.argv:
        sys.stderr.write("run with --stdio\n")
        sys.exit(2)

    for raw in sys.stdin:
        line = raw.lstrip("\ufeff").strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        msg_type = msg.get("type")
        if msg_type == "hello":
            send(
                {
                    "type": "hello",
                    "protocol": PROTOCOL,
                    "name": "powertoys",
                    "commands": COMMANDS,
                }
            )
        elif msg_type == "call":
            send(handle_call(int(msg.get("id", 0)), str(msg.get("command", ""))))
        elif msg_type == "goodbye":
            break


if __name__ == "__main__":
    main()
