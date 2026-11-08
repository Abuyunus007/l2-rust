# AI admin chat: the MCP bridge (`l2-mcp`)

`l2-mcp` lets an AI assistant run the game server's admin commands (D-63). You chat with the
assistant in an MCP client; the client starts `l2-mcp`, and the bridge sends the commands to the
server's admin endpoint, where a virtual GM with the master access level runs them. The game server
needs no change: the bridge uses the same endpoint as the Admin GUI.

```
you  ->  MCP client (Claude Desktop, LM Studio, ...)  ->  l2-mcp  ->  supervisor or game server admin endpoint  ->  world
```

## Tools the assistant gets

| Tool | What it does |
|---|---|
| `server_info` | uptime, players online, memory, tick times, a running shutdown |
| `run_admin_command` | any admin command without slashes (`setlevel 80`, `find_character Amy`, `create_item 57 1000`); refuses the destructive ones |
| `run_destructive_admin_command` | any command, also one that punishes, removes or stops (`ban_char`, `kick`, `kill`, `delete`, `server_shutdown`, ...) |
| `announce` | a message to every player |

"Destructive" is decided by the command word (`l2_mcp::guard`): its parts starting with `ban`,
`jail`, `kick`, `kill`, `del`, `destroy`, `remove`, `clear`, `reset`, `wipe`, `erase`,
`shutdown`, `restart`, `punish`, `take`, `disband`, `unspawn`, `despawn`, `abort`, `logout`,
`disconnect`, `setkarma`, `res`, `unequip`, `drop`.

## Safety: set the destructive tool to "ask every time"

The model cannot approve itself: an MCP client asks you before it calls a tool, unless you told it
to always allow that tool. Allow `server_info`, `run_admin_command` and `announce` if you like;
leave `run_destructive_admin_command` on "ask", and you approve every ban, kill and shutdown.
`--no-destructive` removes that tool altogether.

Every command, with its reason and the first line of the answer, goes to the audit log
(`log/mcp-audit.log` next to where the bridge runs; `--audit-log FILE` or `--audit-log none`).

Small local models (7 to 8 billion parameters) call tools less reliably than large ones: read
what the client says it will run before you approve. Text from the game (player names, chat) can
reach the model through command answers; the bridge tells it that such text is data, but the
approval of destructive commands is the real guard.

## 1. Open the admin endpoint

**With the supervisor (usual setup):** the supervisor already has an endpoint for the Admin GUI.
Its address is `Listen` and its secret `Token` in `bin/supervisor.ini`; the game server's name
there is `game`. Use those, nothing else to change.

**Game server alone:** in `game/config/Admin.ini` set `Enabled = True`, keep
`Bind = 127.0.0.1:7902` and set a `Token`. Restart the game server.

Keep the endpoint on `127.0.0.1` and never put the token into a file you share or commit.

## 2. Settings of the bridge

| Argument | Environment | Default |
|---|---|---|
| `--url ws://host:port` | `L2_MCP_URL` | `ws://127.0.0.1:7902` |
| `--server NAME` (only for a supervisor: `game`) | `L2_MCP_SERVER` | none |
| (token, only from the environment) | `L2_ADMIN_TOKEN` | empty |
| `--audit-log FILE` / `none` | `L2_MCP_AUDIT_LOG` | `log/mcp-audit.log` |
| `--no-destructive` | | off |

The bridge connects on the first command and again after the server restarts, so it can start
before the server.

## 3. Register it in the client

**Claude Desktop** (`Settings > Developer > Edit Config`, file `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "l2-server": {
      "command": "C:\\L2RustServer\\bin\\l2-mcp.exe",
      "args": ["--url", "ws://127.0.0.1:<supervisor port>", "--server", "game",
               "--audit-log", "C:\\L2RustServer\\log\\mcp-audit.log"],
      "env": { "L2_ADMIN_TOKEN": "<the token>" }
    }
  }
}
```

**A local model:** any chat app that runs MCP servers over stdio takes the same three parts
(command, arguments, environment). LM Studio, for example, reads them from its `mcp.json` in the
same `mcpServers` form. Pick a model that supports tool calls.

Then ask, for example: "How many players are online?", "Set Amy to level 60", "Announce that the
server restarts at 22:00", "Ban the character Spambot for a day" (the client asks you first).
