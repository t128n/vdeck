"""Example vDeck plugin implementing the vdeck-plugin/1 stdin/stdout NDJSON contract.

Configure in vdeck.toml:
  [[plugins]]
  id = "example"
  command = "uv"
  args = ["run", "python", "plugins/example.py"]

And bind a key:
  action = { type = "plugin", plugin = "example", command = "toggle_mute", params = { source = "Mic" } }
"""

import json
import sys


def send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main() -> None:
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        msg = json.loads(line)
        kind = msg.get("type")

        if kind == "hello":
            send({
                "type": "hello",
                "protocol": "vdeck-plugin/1",
                "name": "example",
                "commands": ["toggle_mute", "ping"],
            })
        elif kind == "call":
            call_id = msg["id"]
            command = msg.get("command")
            active = bool(msg.get("active", False))

            if command == "toggle_mute":
                new_active = not active
                send({
                    "type": "result",
                    "id": call_id,
                    "ok": True,
                    "active": new_active,
                    "status": "Muted" if new_active else "Live",
                })
            elif command == "ping":
                send({
                    "type": "result",
                    "id": call_id,
                    "ok": True,
                    "status": "Pong",
                })
            else:
                send({
                    "type": "result",
                    "id": call_id,
                    "ok": False,
                    "error": f"unknown command: {command}",
                })
        elif kind == "goodbye":
            break


if __name__ == "__main__":
    main()
