# dist: the server folders

The datapack, the default configuration and the database schema the Rust servers run with. This
is a snapshot of the L2JMobius CT 2.6 High Five release (MIT, see `NOTICE-L2JMobius.txt`),
without the Java script sources and the geodata files. The servers need no other L2JMobius folder.

| Folder | What |
|---|---|
| `game/config/` | game server `.ini`/`.xml` files with the Java defaults, plus the Rust-only `Admin.ini` and `SideBySide.ini` |
| `game/data/` | the datapack (XML, HTML, script pages). `game/data/geodata/` is empty: put the `.l2j` geodata files there |
| `login/` | login server `config/`, `data/` and `banned_ip.cfg` |
| `db_installer/sql/` | the database schema (same tables as Java) |

**Do not put your passwords or tokens here.** Copy `dist/` to `run/` (git-ignored) and edit the
copies there; see the main README.

Update to a newer L2JMobius release (keeps the Rust-only files):

```bash
python tools/sync-dist.py <path to the L2JMobius release's dist folder>
```

## Tests and `dist/`

The tests read this folder (D-55): `l2_testkit::{dist_dir, game_dir, login_dir, sql_dir}` find it
from the repo root, and crates that cannot use `l2-testkit` build the same path from
`CARGO_MANIFEST_DIR`. `L2JMOBIUS_DIST` / `L2JMOBIUS_GAME_DIR` still override it. The database
test schema is built from `db_installer/sql` at compile time (`crates/l2-db/build.rs`, seed rows
left out), so there is no second copy of the tables.

## What still needs L2JMobius

Only things that read the Java sources, not the datapack:

- `codegen` (`L2J_ROOT`): generates code from the Java classes.
- `java-vectors/`: the harness that runs the real Java classes.
- Tests that compare against `.java` files (boat and air ship paths, the handler audit, the
  `PlayerClass` enum, the Castle script links) skip with a message while `dist/` has no `.java`.
