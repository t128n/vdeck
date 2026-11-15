#!/usr/bin/env python3
"""Minimal vDeck plugin starter in Python (vdeck-plugin/1)."""

import json
import sys

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["ping", "toggle_state", "dynamic_deck"]
APP_CONFIG: dict = {}
PLUGIN_CONFIG: dict = {}


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def handle_call(msg_id: int, command: str, active: bool, params: dict) -> dict:
    if command == "ping":
        greeting = PLUGIN_CONFIG.get("greeting", "Pong from Python")
        return {"type": "result", "id": msg_id, "ok": True, "status": str(greeting)}

    if command == "toggle_state":
        next_active = not active
        return {
            "type": "result",
            "id": msg_id,
            "ok": True,
            "active": next_active,
            "status": "Active" if next_active else "Inactive",
        }

    if command == "dynamic_deck":
        deck = {
            "id": "example_py_deck",
            "name": "Example",
            "layers": [
                {
                    "name": "Items",
                    "keys": [
                        {
                            "label": "Ping",
                            "icon": "circle",
                            "accent": "cyan",
                            "action": {
                                "type": "plugin",
                                "plugin": "example_py",
                                "command": "ping",
                            },
                        },
                        {
                            "label": "Back",
                            "icon": "back",
                            "accent": "orange",
                            "action": {"type": "back"},
                        },
                    ],
                }
            ],
        }
        return {
            "type": "result",
            "id": msg_id,
            "ok": True,
            "decks": [deck],
            "open_deck": "example_py_deck",
        }

    return {"type": "result", "id": msg_id, "ok": False, "error": f"unknown command: {command}"}


def main() -> None:
    if "--stdio" not in sys.argv:
        sys.stderr.write("run with --stdio\n")
        sys.exit(2)

    for raw in sys.stdin:
        line = raw.lstrip("\ufeff").strip()
        if not line:
            continue
        msg = json.loads(line)
        msg_type = msg.get("type")

        if msg_type == "hello":
            APP_CONFIG.update(msg.get("config") or {})
            PLUGIN_CONFIG.update(msg.get("plugin_config") or {})
            send({"type": "hello", "protocol": PROTOCOL, "name": "example_py", "commands": COMMANDS})
        elif msg_type == "call":
            send(
                handle_call(
                    msg_id=int(msg.get("id", 0)),
                    command=str(msg.get("command", "")),
                    active=bool(msg.get("active", False)),
                    params=msg.get("params") or {},
                )
            )
        elif msg_type == "goodbye":
            break


if __name__ == "__main__":
    main()
