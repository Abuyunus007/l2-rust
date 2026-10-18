# Configuration, database and datapack

## `.ini` configuration

- **73 `.ini` files, ~1,576 keys**: 5 for login, 25 core game files, 43 in `game/config/Custom/` (customs: faction system, auto-play, offline trade, champions, premium, buffer...).
- Format: Java-properties style `Key = Value`, `#` comments, no sections. Read by `commons/util/ConfigReader` with typed getters + defaults: bool, byte, short, int, long, float, double, string, enum, `Duration`, int arrays with a custom delimiter.
- Values are loaded into **static fields** of 61 config classes (`gameserver/config/*.java`, `config/custom/*.java`, ~6.8k lines), e.g. `RatesConfig.RATE_XP`. Several keys get post-processed (parsing lists like `57,1;...`, maps, derived values).
- Other config: `AccessLevels.xml`, `AdminCommands.xml`, `ClassMaster.xml`, `DynamicExpRates.xml`, `SiegeSchedule.xml`, `SecondaryAuth.xml`, `BotReportPunishments.xml`, `Scripts.xml`, `default-ipconfig.xml` (host/subnet list sent to the LS).

**For Rust:** keep the same files and keys so your existing configs work unchanged (that's your requirement). Each Java config class becomes one Rust struct, loaded once into an immutable `Arc<Config>`, with the Java defaults. Hot reload (`//reload config`) swaps the `Arc`. This work is highly mechanical (key name, type, default, comment) → **generator + Haiku**. Edge cases (post-processing) get flagged per key for Sonnet or review.

## Database

- MySQL/MariaDB through HikariCP + mysql-connector-j. `Database.ini`: URL, user, password, pool size, backup settings.
- **Login: 4 tables. Game: 112 tables** (`dist/db_installer/sql/game/*.sql`). The largest are `characters`, `items`, `character_skills(_save)`, `character_quests`, `clan_data`, `olympiad_*`, `seven_signs*`, `castle*`, `fort*`, plus custom tables (`character_offline_*`, `buffer_schemes`, `rebirth_system`, `character_transmogs`...).
- SQL is hand-written JDBC spread across the code: managers, `Player.store()`/`restore()`, `ItemContainer`, `data/sql/*`. Many statements use MySQL-specific syntax (`REPLACE INTO`, `ON DUPLICATE KEY`, `IF()`).

**For Rust:** `sqlx` with MySQL. The **schema stays identical** to Java, so the Java and Rust servers can run against the same DB (needed for side-by-side comparison and for switching back). Prefer runtime queries with typed rows over compile-time `query!` macros, which need a live DB at build time and are harder for agents/CI. DB access goes behind a persistence layer: game logic never awaits the DB in the middle of a tick (see 04).

## XML datapack (`dist/game/data`, ~85 MB)

| Folder | XML files | Size | Content |
|---|---:|---:|---|
| `stats/npcs` | 139 | 27 MB | NPC templates (stats, skills, drops, AI params) |
| `stats/items` | 198 | 15 MB | item templates |
| `stats/skills` | 95 | 15 MB | skill definitions: effects, conditions, per-level tables |
| `stats/augmentation` | 253 | 8 MB | |
| `stats/players` | 223 | 4.5 MB | class templates, exp tables, initial equipment |
| `stats/pets`, `transformations`, `armorsets`, `cubics`, `fishing` | ~200 | ~6.5 MB | |
| `buylists` | 760 | 3.4 MB | NPC shops |
| `multisell` | 373 | 5.4 MB | |
| `spawns` | 187 | 4.3 MB | spawn lists |
| `zones` | 48 | 1.3 MB | zone polygons |
| `teleporters`, `instances`, `mapregion`, `lang` | ~370 | ~1.2 MB | |
| `scripts` (XML parts) | 20 | | |
| `xsd` | 76 `.xsd` | 470 KB | **XSD schemas for the XML file types**: useful for generating serde structs |
| `html` | (`.htm`) | 8 MB | NPC dialogs, loaded by `HtmCache` |

- 63 loaders implement `IXmlReader` (DOM parsing, attribute-by-attribute into `StatSet` or holders).
- Skills and items are the complicated ones: `DocumentSkill`/`DocumentBase` (~3.7k lines) parse tables (`#name` references per level), conditions (88 condition classes) and effects (handler names that map to `scripts/handlers/skill`).
- **For Rust:** `quick-xml` + serde structs per file type, generated from the XSDs as far as possible. The data is loaded once into immutable `Arc` tables at startup. Each loader is a well-defined task (input XML + XSD → struct + count check matching what the Java log reports on startup: "Loaded N items") → good **Haiku** work after a reference loader exists.

## Logging
`log.cfg` (java.util.logging) + custom formatters (chat, item, enchant, accounting logs in `log/`). In Rust: `tracing` with file appenders, keeping the same separate log files (chat, items, GM audit), since admins use them.
