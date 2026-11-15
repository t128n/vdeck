// Minimal vDeck plugin starter in TypeScript (vdeck-plugin/1).
// Run with Bun (`bun examples/plugin_starter.ts --stdio`),
// Node 22+ (`node --experimental-strip-types examples/plugin_starter.ts --stdio`),
// or Deno (`deno run -A examples/plugin_starter.ts --stdio`).

import * as readline from "node:readline";

const PROTOCOL = "vdeck-plugin/1";
const COMMANDS = ["ping", "toggle_state", "dynamic_deck"] as const;

interface HostHello {
  type: "hello";
  protocol: string;
  config?: Record<string, unknown>;
  plugin_config?: Record<string, unknown>;
}

interface HostCall {
  type: "call";
  id: number;
  command: string;
  active: boolean;
  params?: Record<string, unknown>;
}

interface HostGoodbye {
  type: "goodbye";
}

type HostMsg = HostHello | HostCall | HostGoodbye;

let appConfig: Record<string, unknown> = {};
let pluginConfig: Record<string, unknown> = {};

function send(msg: Record<string, unknown>): void {
  process.stdout.write(`${JSON.stringify(msg)}\n`);
}

function handleCall(msg: HostCall): Record<string, unknown> {
  switch (msg.command) {
    case "ping": {
      const greeting = String(pluginConfig.greeting ?? "Pong from TypeScript");
      return { type: "result", id: msg.id, ok: true, status: greeting };
    }
    case "toggle_state": {
      const nextActive = !msg.active;
      return {
        type: "result",
        id: msg.id,
        ok: true,
        active: nextActive,
        status: nextActive ? "Active" : "Inactive",
      };
    }
    case "dynamic_deck": {
      return {
        type: "result",
        id: msg.id,
        ok: true,
        open_deck: "example_ts_deck",
        decks: [
          {
            id: "example_ts_deck",
            name: "Example",
            layers: [
              {
                name: "Items",
                keys: [
                  {
                    label: "Ping",
                    icon: "circle",
                    accent: "cyan",
                    action: {
                      type: "plugin",
                      plugin: "example_ts",
                      command: "ping",
                    },
                  },
                  {
                    label: "Back",
                    icon: "back",
                    accent: "orange",
                    action: { type: "back" },
                  },
                ],
              },
            ],
          },
        ],
      };
    }
    default:
      return {
        type: "result",
        id: msg.id,
        ok: false,
        error: `unknown command: ${msg.command}`,
      };
  }
}

if (!process.argv.includes("--stdio")) {
  process.stderr.write("run with --stdio\n");
  process.exit(2);
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

rl.on("line", (raw: string) => {
  const line = raw.replace(/^\uFEFF/, "").trim();
  if (!line) return;

  let msg: HostMsg;
  try {
    msg = JSON.parse(line) as HostMsg;
  } catch {
    return;
  }

  if (msg.type === "hello") {
    appConfig = msg.config ?? {};
    pluginConfig = msg.plugin_config ?? {};
    send({ type: "hello", protocol: PROTOCOL, name: "example_ts", commands: COMMANDS });
  } else if (msg.type === "call") {
    send(handleCall(msg));
  } else if (msg.type === "goodbye") {
    rl.close();
  }
});
