#!/usr/bin/env python3
"""Build a standalone server package (like an L2J release folder) from this repo.

Usage:
    cargo build --release -p login-server -p game-server -p l2-supervisor -p l2-admin-gui -p l2-db-installer
    python tools/package.py --geodata <folder with *.l2j> [--out <folder>] [--zip]

Layout of the package:
    bin/             the programs and supervisor.ini (the admin GUI finds it next to itself)
    login/           login server: config/, data/, banned_ip.cfg   (from dist/login)
    game/            game server: config/, data/ (datapack + geodata) (from dist/)
    db_installer/    sql/ and config/ for the database installer
    Start-AdminGUI.cmd, Install-Database.cmd, README.txt

The package gets fresh random admin tokens (Admin.ini of both servers, supervisor.ini). It is a
local build artifact: never commit it, never share it with the tokens in it.
"""

import argparse
import secrets
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DIST = REPO / "dist"
RELEASE = REPO / "target" / "release"
EXE = ".exe" if sys.platform == "win32" else ""
PROGRAMS = ["login-server", "game-server", "l2-supervisor", "l2-admin-gui", "l2-db-installer"]


def set_keys(path: Path, values: dict[str, str]) -> None:
    """Change `Key = value` lines in place (comments and line endings kept)."""
    data = path.read_bytes().decode("utf-8")
    out = []
    seen = set()
    for line in data.splitlines(keepends=True):
        body = line.rstrip("\r\n")
        end = line[len(body):]
        key = body.split("=", 1)[0].strip() if "=" in body and not body.lstrip().startswith("#") else None
        if key in values:
            out.append(f"{key} = {values[key]}{end}")
            seen.add(key)
        else:
            out.append(line)
    for key in values.keys() - seen:
        out.append(f"{key} = {values[key]}\n")
    path.write_bytes("".join(out).encode("utf-8"))


def supervisor_ini(token: str, login_token: str, game_token: str) -> str:
    return f"""# Supervisor settings of this package. The admin GUI reads Listen and Token from here.
# Relative paths are relative to this file's folder (bin/).
Listen = 127.0.0.1:7900
Token = {token}
LogBufferLines = 10000
StopTimeoutSecs = 30
RestartDelaySecs = 10
Servers = login, game

Server.login.Command = ./login-server{EXE}
Server.login.Args =
Server.login.WorkingDir = ../login
Server.login.AdminEndpoint = 127.0.0.1:7901
Server.login.AdminToken = {login_token}
Server.login.Autostart = False
Server.login.RestartOnExitCode = 2

Server.game.Command = ./game-server{EXE}
Server.game.Args = --root .
Server.game.WorkingDir = ../game
Server.game.AdminEndpoint = 127.0.0.1:7902
Server.game.AdminToken = {game_token}
Server.game.Autostart = False
Server.game.RestartOnExitCode = 2
"""


README = """L2 Rust server (High Five) - standalone package
=================================================

What is in here
  bin\\           the programs: login-server, game-server, l2-supervisor, l2-admin-gui,
                 l2-db-installer, and supervisor.ini
  login\\         login server settings (config\\) and data
  game\\          game server settings (config\\) and the datapack (data\\, geodata included)
  db_installer\\  the database schema (sql\\)

You need
  - MariaDB or MySQL (running, with a user that may create a database)
  - a Lineage 2 High Five client

First start
  1. Install-Database.cmd       creates the database "l2jmobiush5" (asks host, user, password).
  2. Database settings          if your database user or password is not root / empty, open the
                                admin GUI -> Config, and set Login and Password in
                                login\\config\\Database.ini and game\\config\\Database.ini
                                (or edit those two files with a text editor).
  3. Start-AdminGUI.cmd         opens the admin GUI. It starts the supervisor in the background,
                                then press "Start all" in the Servers view (login first, then game).
  4. Client                     point the client's l2.ini / hosts entry at 127.0.0.1, port 2106.

Ports (Java defaults): login 2106 (clients), 9014 (game servers); game 7777. Do not run a Java
L2J login or game server with the same ports at the same time.

Closing the admin GUI keeps the servers running. Open it again to reconnect; "Stop all" stops
them cleanly. Logs: login\\log, game\\log, bin\\log.

The admin tokens in bin\\supervisor.ini and the two Admin.ini files were made for this package:
keep them private.
"""

START_GUI = "@echo off\r\ncd /d \"%~dp0bin\"\r\nstart \"\" \"l2-admin-gui.exe\"\r\n"
INSTALL_DB = (
    "@echo off\r\ncd /d \"%~dp0bin\"\r\n"
    "\"l2-db-installer.exe\" --sql \"%~dp0db_installer\\sql\"\r\n"
    "if errorlevel 1 pause\r\n"
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--geodata", required=True, type=Path, help="folder with the *.l2j geodata files")
    ap.add_argument("--out", type=Path, default=REPO / "release" / "L2RustServer")
    ap.add_argument("--zip", action="store_true", help="also write <out>.zip")
    args = ap.parse_args()

    missing = [p for p in PROGRAMS if not (RELEASE / f"{p}{EXE}").is_file()]
    if missing:
        print("missing release builds:", ", ".join(missing))
        print("run: cargo build --release " + " ".join(f"-p {p}" for p in PROGRAMS))
        return 1
    geo = sorted(args.geodata.glob("*.l2j"))
    if not geo:
        print(f"no *.l2j files in {args.geodata}")
        return 1

    out: Path = args.out.resolve()
    if out.exists():
        shutil.rmtree(out)
    (out / "bin").mkdir(parents=True)
    for p in PROGRAMS:
        shutil.copy2(RELEASE / f"{p}{EXE}", out / "bin" / f"{p}{EXE}")

    shutil.copytree(DIST / "login", out / "login")
    shutil.copytree(DIST / "game" / "config", out / "game" / "config")
    shutil.copytree(DIST / "game" / "data", out / "game" / "data")
    shutil.copytree(DIST / "db_installer", out / "db_installer")
    for f in geo:
        shutil.copy2(f, out / "game" / "data" / "geodata" / f.name)

    token, login_token, game_token = (secrets.token_urlsafe(24) for _ in range(3))
    set_keys(out / "login" / "config" / "Admin.ini", {"Enabled": "True", "Token": login_token})
    set_keys(out / "game" / "config" / "Admin.ini", {"Enabled": "True", "Token": game_token})
    (out / "bin" / "supervisor.ini").write_text(supervisor_ini(token, login_token, game_token), encoding="utf-8", newline="\n")
    (out / "README.txt").write_text(README.replace("\n", "\r\n"), encoding="utf-8", newline="")
    (out / "Start-AdminGUI.cmd").write_text(START_GUI, encoding="utf-8", newline="")
    (out / "Install-Database.cmd").write_text(INSTALL_DB, encoding="utf-8", newline="")

    size = sum(f.stat().st_size for f in out.rglob("*") if f.is_file())
    print(f"package: {out} ({size / 1e9:.2f} GB, {len(geo)} geodata regions)")
    if args.zip:
        archive = shutil.make_archive(str(out), "zip", root_dir=out.parent, base_dir=out.name)
        print(f"zip: {archive} ({Path(archive).stat().st_size / 1e9:.2f} GB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
