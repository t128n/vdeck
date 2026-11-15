#!/usr/bin/env python3
"""vDeck Markets / S&P 500 & NASDAQ ticker plugin (vdeck-plugin/1)."""

from concurrent.futures import ThreadPoolExecutor
import json
import sys
import urllib.parse
import urllib.request

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["ticker", "board", "quote"]
DEFAULT_SYMBOLS = [
    ("^GSPC", "S&P 500"),
    ("^IXIC", "NASDAQ"),
    ("^DJI", "DOW"),
    ("BTC-USD", "BTC"),
    ("NVDA", "NVDA"),
    ("AAPL", "AAPL"),
    ("MSFT", "MSFT"),
]
PLUGIN_CONFIG: dict = {}


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def fetch_quote(entry: tuple[str, str]) -> dict | None:
    symbol, label = entry
    encoded = urllib.parse.quote(symbol, safe="")
    url = f"https://query1.finance.yahoo.com/v8/finance/chart/{encoded}?range=1d&interval=1d"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        with urllib.request.urlopen(req, timeout=6.0) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        meta = data["chart"]["result"][0]["meta"]
        price = float(meta["regularMarketPrice"])
        prev = float(meta.get("chartPreviousClose") or price)
        pct = ((price - prev) / prev * 100.0) if prev else 0.0
        return {"symbol": symbol, "label": label, "price": price, "pct": pct}
    except Exception:
        return None


def resolve_symbols() -> list[tuple[str, str]]:
    configured = PLUGIN_CONFIG.get("symbols")
    if isinstance(configured, list) and configured:
        out: list[tuple[str, str]] = []
        for item in configured:
            if isinstance(item, str):
                out.append((item, item.lstrip("^")))
            elif isinstance(item, dict) and item.get("symbol"):
                sym = str(item["symbol"])
                out.append((sym, str(item.get("label") or sym.lstrip("^"))))
        if out:
            return out
    return DEFAULT_SYMBOLS


def fetch_all() -> list[dict]:
    entries = resolve_symbols()
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(fetch_quote, entries))
    quotes = [q for q in results if q is not None]
    if not quotes:
        raise RuntimeError("Failed to fetch market quotes")
    return quotes


def format_line(q: dict) -> str:
    sign = "+" if q["pct"] >= 0 else ""
    pct_color = "\x1b[92m" if q["pct"] >= 0 else "\x1b[91m"
    return f"\x1b[97m{q['label']}\x1b[0m {q['price']:,.2f} {pct_color}{sign}{q['pct']:.2f}%\x1b[0m"


def build_board_deck(quotes: list[dict]) -> dict:
    keys: list[dict] = []
    q_iter = iter(quotes)
    for slot in range(9):
        if slot == 6:
            keys.append(
                {"label": "Back", "icon": "back", "accent": "orange", "action": {"type": "back"}}
            )
            continue
        if slot == 8:
            keys.append(
                {
                    "label": "Refresh",
                    "icon": "play",
                    "accent": "cyan",
                    "action": {"type": "plugin", "plugin": "markets", "command": "board"},
                }
            )
            continue
        q = next(q_iter, None)
        if q is None:
            keys.append(
                {"label": "", "icon": "circle", "accent": "cyan", "action": {"type": "none"}}
            )
            continue
        sign = "+" if q["pct"] >= 0 else ""
        keys.append(
            {
                "label": f"{q['label']} {sign}{q['pct']:.1f}%",
                "icon": "lucide:trending-up" if q["pct"] >= 0 else "lucide:trending-down",
                "accent": "green" if q["pct"] >= 0 else "red",
                "close": False,
                "action": {
                    "type": "plugin",
                    "plugin": "markets",
                    "command": "quote",
                    "params": {"symbol": q["symbol"], "label": q["label"]},
                },
            }
        )
    return {"id": "markets_board", "name": "Markets", "layers": [{"name": "Live", "keys": keys}]}


def handle_call(msg_id: int, command: str, active: bool, params: dict) -> dict:
    try:
        speed = float(params.get("speed", PLUGIN_CONFIG.get("speed", 28.0)))

        if command == "ticker":
            if active:
                send({"type": "news_strip", "show": False})
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": True,
                    "active": False,
                    "status": "Markets Off",
                }
            quotes = fetch_all()
            items = [format_line(q) for q in quotes]
            send(
                {
                    "type": "news_strip",
                    "show": True,
                    "title": "MKT",
                    "accent": "green",
                    "items": items,
                    "speed": speed,
                }
            )
            send(
                {
                    "type": "toast",
                    "title": quotes[0]["label"],
                    "detail": f"{quotes[0]['price']:,.2f} ({quotes[0]['pct']:+.2f}%)",
                    "icon": "lucide:trending-up" if quotes[0]["pct"] >= 0 else "lucide:trending-down",
                    "accent": "green" if quotes[0]["pct"] >= 0 else "red",
                }
            )
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "active": True,
                "status": "Markets On",
            }

        if command == "board":
            quotes = fetch_all()
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [build_board_deck(quotes)],
                "open_deck": "markets_board",
                "status": f"{len(quotes)} quotes",
            }

        if command == "quote":
            sym = str(params.get("symbol", "^GSPC"))
            label = str(params.get("label", sym.lstrip("^")))
            q = fetch_quote((sym, label))
            if q is None:
                return {"type": "result", "id": msg_id, "ok": False, "error": f"No quote for {sym}"}
            send(
                {
                    "type": "toast",
                    "title": q["label"],
                    "detail": f"{q['price']:,.2f} ({q['pct']:+.2f}%)",
                    "icon": "lucide:trending-up" if q["pct"] >= 0 else "lucide:trending-down",
                    "accent": "green" if q["pct"] >= 0 else "red",
                }
            )
            return {"type": "result", "id": msg_id, "ok": True, "status": format_line(q)}

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
            PLUGIN_CONFIG.clear()
            PLUGIN_CONFIG.update(msg.get("plugin_config") or {})
            send(
                {
                    "type": "hello",
                    "protocol": PROTOCOL,
                    "name": "markets",
                    "commands": COMMANDS,
                }
            )
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
