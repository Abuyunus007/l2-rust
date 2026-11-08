# AI admin chat: `l2-ai` (local model) and `l2-mcp` (MCP bridge)

An AI assistant runs the game server's admin commands (D-63), in one of three ways:

- **Admin GUI, view "AI assistant"** (the simplest): the chat in the GUI, through the GUI's own
  connection to the supervisor. Pick the model server (Ollama: `http://127.0.0.1:11434`) and the
  model, write what you want done. A ban, kick, kill or shutdown opens a dialog: "Run it" or
  "Don't run". The settings are kept with the GUI's; the audit log is `log/mcp-audit.log` in the
  run folder.
- **`l2-ai`**: a chat in the terminal with a small model on your own PC (llama.cpp's
  `llama-server` or Ollama); no other app is needed. See "Local model with `l2-ai`" below.
- **`l2-mcp`**: an MCP server for an MCP client (Claude Desktop, LM Studio, ...).

Both send the commands to the server's admin endpoint, where a virtual GM with the master access
level runs them; it is the endpoint of the Admin GUI.

```
you  ->  l2-ai  ->  llama-server (the model)
           \->  supervisor or game server admin endpoint  ->  world
you  ->  MCP client  ->  l2-mcp  ->  supervisor or game server admin endpoint  ->  world
```

## Tools the assistant gets

| Tool | What it does |
|---|---|
| `server_info` | uptime, players online, memory, tick times, a running shutdown |
| `command_help` | looks commands up in the catalog (`l2_mcp::catalog`, 43 commands): syntax, meaning, example, whether they need a target |
| `run_admin_command` | any admin command without slashes (`set_level 80`, `find_character Amy`); refuses the destructive ones |
| `run_destructive_admin_command` | any command, also one that punishes, removes or stops (`ban_char`, `kick`, `kill`, `server_shutdown`, ...) |
| `announce` | a message to every player |

Both command tools take an optional `target`: the name of an online character. A command on "the
selected target" (`set_level`, `give_item_target`, `add_skill`, `sethero`, ...) runs with that
character as the GM's target (`GameAction::CommandOn`). The virtual GM has no position and no
bag of its own: `teleportto`, `move_to`, `goto`, `recall` and `create_item` do nothing useful
through the endpoint, and the catalog leaves them out.

"Destructive" is decided by the command word (`l2_mcp::guard`): its parts starting with `ban`,
`jail`, `kick`, `kill`, `del`, `destroy`, `remove`, `clear`, `reset`, `wipe`, `erase`,
`shutdown`, `restart`, `punish`, `take`, `disband`, `unspawn`, `despawn`, `logout`,
`disconnect`, `setkarma`, `respawn`, `unequip`, `drop`.

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

## Local model with `l2-ai`

`l2-ai` is a chat in the terminal. You write what you want ("set Amy to level 60", "how many
players are online?"); the model picks the command, `l2-ai` runs it and the model says what
happened. Before a command that punishes, removes or stops something, `l2-ai` itself asks you
`Run it? (y/j = yes ...)`: the model cannot answer for you. Everything goes to the audit log.

1. **The model server:** llama.cpp for Windows with CUDA (GitHub, `ggml-org/llama.cpp`, Releases,
   the `win-cuda` zip), and a model as a `Q4_K_M` GGUF, e.g. Qwen3.5-9B (about 5.6 GB). Start it:
   ```
   llama-server -m Qwen3.5-9B-Q4_K_M.gguf --jinja -ngl 99 --port 8080
   ```
   `--jinja` turns on the model's tool calls; `-ngl 99` puts the model on the GPU.
   **Ollama** works as well and needs no `llama-server`: pull the model
   (`ollama pull hf.co/unsloth/Qwen3.5-9B-GGUF:Q4_K_M`) and start `l2-ai` with
   `--llm-url http://127.0.0.1:11434 --model hf.co/unsloth/Qwen3.5-9B-GGUF:Q4_K_M`.
2. **The admin endpoint:** as in step 1 above (supervisor `Listen`/`Token`, or `Admin.ini`).
3. **Start the chat** (PowerShell, in the folder of the server package):
   ```
   $env:L2_ADMIN_TOKEN = "<the token>"
   bin\l2-ai.exe --url ws://127.0.0.1:<port> --server game
   ```
   `/reset` forgets the conversation, `/exit` ends it.

| Argument | Environment | Default |
|---|---|---|
| `--llm-url http://host:port` | `L2_AI_LLM_URL` | `http://127.0.0.1:8080` |
| `--model NAME` | `L2_AI_MODEL` | `local` (Ollama needs the real name) |
| `--url`, `--server`, `--audit-log`, `--no-destructive` | as for `l2-mcp` | |

A small model can pick a wrong command or a wrong number: read its answer, and read the question
before you say yes. Ask for one thing at a time; name the character the command is for.

**Test a model** before you rely on it: `crates/l2-mcp/tests/live_model.rs` gives it five
typical requests (a level, players online, Adena for a character, an announcement, a ban the
administrator declines) against a stand-in server:

```
$env:L2_AI_LIVE_URL = "http://127.0.0.1:11434"
$env:L2_AI_LIVE_MODEL = "hf.co/unsloth/Qwen3.5-9B-GGUF:Q4_K_M"
cargo test -p l2-mcp --test live_model -- --nocapture
```

Qwen3.5-9B (Q4_K_M, Ollama, RTX 4070 Ti) got all five right in 1 to 5 seconds each (2026-11-08,
one run; a model does not answer the same way every time).
