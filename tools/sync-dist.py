#!/usr/bin/env python3
"""Refresh the repository's own server folders (dist/) from an L2JMobius CT 2.6 High Five release.

Usage:
    python tools/sync-dist.py <path to the L2JMobius dist folder>

The source is the release's `dist` folder (the one with `game/`, `login/` and `db_installer/`).
What is copied (byte for byte, line endings kept):
    game/config/**            the Java game server .ini and .xml files (defaults, no secrets;
                              without hexid.txt)
    game/data/**              the datapack, without the Java script sources (*.java) and without
                              the geodata files (geodata is a separate download)
    login/config/**           the Java login server .ini files
    login/data/**             servername.xml and the other login data
    login/banned_ip.cfg
    db_installer/sql/**       the database schema (the Rust servers use the same tables)

Rust-only example files (Admin.ini, SideBySide.ini) are added next to the Java configs; they are
kept when they already exist. Files under dist/ that no longer exist in the release are removed,
except those Rust-only files. Run it again after updating to a newer L2JMobius release.
"""

import os
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DIST = REPO / "dist"

# (source dir relative to the release, destination relative to dist/)
TREES = [
    ("game/config", "game/config"),
    ("game/data", "game/data"),
    ("login/config", "login/config"),
    ("login/data", "login/data"),
    ("db_installer/sql", "db_installer/sql"),
    ("db_installer/config", "db_installer/config"),
]
FILES = [("login/banned_ip.cfg", "login/banned_ip.cfg")]

# Rust-only config files (D-36): never overwritten or removed by a sync.
RUST_ONLY = {
    "game/config/Admin.ini": "Enabled = False\nBind = 127.0.0.1:7902\n"
    "# Shared secret of the admin endpoint (the supervisor's Server.game.AdminToken).\n"
    "# Set it in your own copy of this folder; never commit a real token.\n"
    "Token =\n",
    "login/config/Admin.ini": "Enabled = False\nBind = 127.0.0.1:7901\n"
    "# Shared secret of the admin endpoint (the supervisor's Server.login.AdminToken).\n"
    "# Set it in your own copy of this folder; never commit a real token.\n"
    "Token =\n",
    # Start scripts of the Rust database installer (T-DBI-01); the release's own are Java ones.
    "db_installer/DatabaseInstaller.cmd": "@echo off\r\n"
    "rem Starts the database installer (GUI, or console with --console).\r\n"
    'cd /d "%~dp0"\r\n'
    'if exist "l2-db-installer.exe" (\r\n'
    '  "l2-db-installer.exe" --sql "%~dp0sql" %*\r\n'
    "  goto :eof\r\n"
    ")\r\n"
    'if exist "..\\..\\target\\release\\l2-db-installer.exe" (\r\n'
    '  "..\\..\\target\\release\\l2-db-installer.exe" --sql "%~dp0sql" %*\r\n'
    "  goto :eof\r\n"
    ")\r\n"
    "echo l2-db-installer.exe not found. Build it: cargo build --release -p l2-db-installer\r\n"
    "pause\r\n",
    "db_installer/DatabaseInstaller.sh": "#!/bin/sh\n"
    "# Starts the database installer (GUI, or console with --console).\n"
    'cd "$(dirname "$0")" || exit 1\n'
    "for exe in ./l2-db-installer ../../target/release/l2-db-installer; do\n"
    '    if [ -x "$exe" ]; then\n'
    '        exec "$exe" --sql "$(pwd)/sql" "$@"\n'
    "    fi\n"
    "done\n"
    'echo "l2-db-installer not found. Build it: cargo build --release -p l2-db-installer" >&2\n'
    "exit 1\n",
}
RUST_ONLY_COPIES = {
    "game/config/SideBySide.ini": REPO / "crates/game-server/config-examples/SideBySide.ini",
}


def skip(rel: str) -> bool:
    if rel.endswith(".java"):
        return True
    # The game server's registration key at the login server: each install makes its own.
    if rel.endswith("/hexid.txt"):
        return True
    # Geodata is a separate download; keep only the folder's readme.
    if rel.startswith("game/data/geodata/") and not rel.endswith("Readme.txt"):
        return True
    return False


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 1
    src = Path(sys.argv[1]).resolve()
    if not (src / "game" / "data").is_dir() or not (src / "login").is_dir():
        print(f"{src} does not look like an L2JMobius dist folder (game/data, login missing)")
        return 1

    wanted: set[str] = set()
    copied = 0
    for s, d in TREES:
        for root, _dirs, files in os.walk(src / s):
            for name in files:
                path = Path(root) / name
                rel = (Path(d) / path.relative_to(src / s)).as_posix()
                if skip(rel):
                    continue
                wanted.add(rel)
                dest = DIST / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                if not dest.exists() or dest.read_bytes() != path.read_bytes():
                    shutil.copyfile(path, dest)
                    copied += 1
    for s, d in FILES:
        if (src / s).is_file():
            wanted.add(d)
            (DIST / d).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src / s, DIST / d)

    for rel, text in RUST_ONLY.items():
        wanted.add(rel)
        dest = DIST / rel
        if not dest.exists():
            dest.write_text(text, encoding="utf-8", newline="\n")
    for rel, example in RUST_ONLY_COPIES.items():
        wanted.add(rel)
        dest = DIST / rel
        if not dest.exists():
            shutil.copyfile(example, dest)

    removed = 0
    for path in sorted(DIST.rglob("*")):
        if path.is_file():
            rel = path.relative_to(DIST).as_posix()
            if rel in ("README.md", "NOTICE-L2JMobius.txt") or rel in wanted:
                continue
            path.unlink()
            removed += 1
    for path in sorted(DIST.rglob("*"), reverse=True):
        if path.is_dir() and not any(path.iterdir()):
            path.rmdir()

    print(f"dist/: {len(wanted)} files, {copied} copied or changed, {removed} removed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
