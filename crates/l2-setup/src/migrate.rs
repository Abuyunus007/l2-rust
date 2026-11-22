//! Brings an existing L2JMobius H5 database up to this server's schema without losing data.
//!
//! The installer's scripts `DROP` and recreate some tables (`account_premium`,
//! `character_offline_play`, the spawn tables ...), so running them on a database in use would
//! delete player data. A migration never drops, replaces or empties anything:
//! - a table the database does not have: its script runs (table and default rows);
//! - a table it has: only the columns it lacks are added (`ALTER TABLE ... ADD COLUMN` with the
//!   script's definition, at the script's place: some inserts give no column list); its
//!   `INSERT IGNORE` default rows run only when the table is empty (a table with rows keeps
//!   them as they are: a raid boss the owner removed does not come back); `DROP`, `REPLACE` and
//!   plain `INSERT` are left out.
//!
//! [`plan`] is pure; [`existing_schema`] reads the database and [`apply`] runs a plan.

use std::collections::{BTreeMap, BTreeSet};

use l2_db_installer::installer::Script;
use sqlx::mysql::MySqlConnection;
use sqlx::{AssertSqlSafe, Row};

/// The tables of a database, lower case, each with its columns, lower case.
pub type Schema = BTreeMap<String, BTreeSet<String>>;

/// One table of a `CREATE TABLE` statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    pub name: String,
    /// Each column: its name and its whole definition (`` `level` TINYINT UNSIGNED DEFAULT NULL``).
    pub columns: Vec<(String, String)>,
}

/// What a statement of a script does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    DropTable,
    CreateTable(TableDef),
    /// `INSERT IGNORE`: never overwrites a row.
    InsertIgnore,
    /// `INSERT` (fails on an existing key) and `REPLACE` (overwrites a row).
    Write,
    Other,
}

pub fn classify(sql: &str) -> Kind {
    let words: Vec<String> = sql
        .split_whitespace()
        .take(3)
        .map(str::to_ascii_uppercase)
        .collect();
    match words.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["DROP", "TABLE", ..] => Kind::DropTable,
        ["CREATE", "TABLE", ..] => parse_create_table(sql).map_or(Kind::Other, Kind::CreateTable),
        ["INSERT", "IGNORE", ..] => Kind::InsertIgnore,
        ["INSERT", ..] | ["REPLACE", ..] => Kind::Write,
        _ => Kind::Other,
    }
}

/// The table and the columns of a `CREATE TABLE [IF NOT EXISTS] name (...)` statement.
pub fn parse_create_table(sql: &str) -> Option<TableDef> {
    let open = sql.find('(')?;
    let head: Vec<&str> = sql[..open].split_whitespace().collect();
    let name = head.last()?.trim_matches('`').to_owned();
    let is_create_table =
        head.first()?.eq_ignore_ascii_case("CREATE") && head.get(1)?.eq_ignore_ascii_case("TABLE");
    if name.is_empty() || !is_create_table {
        return None;
    }
    let body = matching_body(&sql[open..])?;
    let mut columns = Vec::new();
    for part in split_top_level(body) {
        let part = part.trim();
        let first = part.split_whitespace().next().unwrap_or("");
        let keyword = first.to_ascii_uppercase();
        if part.is_empty()
            || matches!(
                keyword.as_str(),
                "PRIMARY"
                    | "KEY"
                    | "INDEX"
                    | "UNIQUE"
                    | "CONSTRAINT"
                    | "FOREIGN"
                    | "FULLTEXT"
                    | "SPATIAL"
                    | "CHECK"
            )
        {
            continue;
        }
        let column = if let Some(rest) = part.strip_prefix('`') {
            rest.split('`').next().unwrap_or("").to_owned()
        } else {
            first.to_owned()
        };
        if !column.is_empty() {
            columns.push((column, part.to_owned()));
        }
    }
    Some(TableDef { name, columns })
}

/// The text inside the parentheses that `text` starts with.
fn matching_body(text: &str) -> Option<&str> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth == 0 {
                    return text.get(1..i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `body` split at its commas outside parentheses and quotes (`DECIMAL(10,2)`, `ENUM('a','b')`
/// stay whole).
fn split_top_level(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in body.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

/// One thing a migration does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A statement of a script, as it is.
    Run { script: String, sql: String },
    /// `ALTER TABLE ... ADD COLUMN ...`.
    AddColumn {
        table: String,
        column: String,
        sql: String,
    },
}

impl Step {
    pub fn sql(&self) -> &str {
        match self {
            Step::Run { sql, .. } | Step::AddColumn { sql, .. } => sql,
        }
    }
}

/// What a migration will do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// The tables it creates.
    pub new_tables: Vec<String>,
    /// The columns it adds: table, column.
    pub new_columns: Vec<(String, String)>,
    /// The tables that are there and complete.
    pub complete_tables: usize,
}

impl Plan {
    /// Nothing to change.
    pub fn is_empty(&self) -> bool {
        self.new_tables.is_empty() && self.new_columns.is_empty() && self.steps.is_empty()
    }
}

/// The table a script makes (lower case), if it makes one.
pub fn script_table(script: &Script) -> Option<String> {
    script.statements.iter().find_map(|s| match classify(s) {
        Kind::CreateTable(def) => Some(def.name.to_ascii_lowercase()),
        _ => None,
    })
}

/// The migration of `existing` to the schema of `scripts` (see the module doc). `with_rows`:
/// the existing tables that hold rows.
pub fn plan(scripts: &[Script], existing: &Schema, with_rows: &BTreeSet<String>) -> Plan {
    let mut plan = Plan::default();
    for script in scripts {
        let kinds: Vec<Kind> = script.statements.iter().map(|s| classify(s)).collect();
        let Some(table) = kinds.iter().find_map(|k| match k {
            Kind::CreateTable(def) => Some(def.clone()),
            _ => None,
        }) else {
            // A script without a table of its own (none ships): nothing is certain to be safe.
            continue;
        };
        match existing.get(&table.name.to_ascii_lowercase()) {
            None => {
                plan.new_tables.push(table.name.clone());
                for sql in &script.statements {
                    plan.steps.push(Step::Run {
                        script: script.name.clone(),
                        sql: sql.clone(),
                    });
                }
            }
            Some(columns) => {
                let mut complete = true;
                for (i, (column, definition)) in table.columns.iter().enumerate() {
                    if !columns.contains(&column.to_ascii_lowercase()) {
                        complete = false;
                        // At the script's place, as a fresh install has it.
                        let place = match i.checked_sub(1).and_then(|p| table.columns.get(p)) {
                            Some((previous, _)) => format!(" AFTER `{previous}`"),
                            None => " FIRST".to_owned(),
                        };
                        plan.new_columns.push((table.name.clone(), column.clone()));
                        plan.steps.push(Step::AddColumn {
                            table: table.name.clone(),
                            column: column.clone(),
                            sql: format!("ALTER TABLE `{}` ADD COLUMN {definition}{place}", table.name),
                        });
                    }
                }
                if complete {
                    plan.complete_tables += 1;
                }
                // Default rows only into an empty table.
                if with_rows.contains(&table.name.to_ascii_lowercase()) {
                    continue;
                }
                for (sql, kind) in script.statements.iter().zip(&kinds) {
                    if *kind == Kind::InsertIgnore {
                        plan.steps.push(Step::Run {
                            script: script.name.clone(),
                            sql: sql.clone(),
                        });
                    }
                }
            }
        }
    }
    plan
}

/// The tables of `database` with their columns (lower case).
pub async fn existing_schema(conn: &mut MySqlConnection, database: &str) -> Result<Schema, sqlx::Error> {
    let rows =
        sqlx::query("SELECT TABLE_NAME, COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ?")
            .bind(database)
            .fetch_all(&mut *conn)
            .await?;
    let mut schema = Schema::new();
    for row in rows {
        let table: String = row.try_get(0)?;
        let column: String = row.try_get(1)?;
        schema
            .entry(table.to_ascii_lowercase())
            .or_default()
            .insert(column.to_ascii_lowercase());
    }
    Ok(schema)
}

/// The tables of `tables` that hold at least one row.
pub async fn tables_with_rows(
    conn: &mut MySqlConnection,
    tables: impl IntoIterator<Item = String>,
) -> Result<BTreeSet<String>, sqlx::Error> {
    let mut with_rows = BTreeSet::new();
    for table in tables {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM `{}` LIMIT 1)",
            table.replace('`', "")
        );
        let any: i64 = sqlx::query_scalar(AssertSqlSafe(sql))
            .fetch_one(&mut *conn)
            .await?;
        if any != 0 {
            with_rows.insert(table);
        }
    }
    Ok(with_rows)
}

/// The login tables are there (an L2J login database).
pub fn has_login_tables(schema: &Schema) -> bool {
    schema.contains_key("accounts")
}

/// The game tables are there (an L2J game database).
pub fn has_game_tables(schema: &Schema) -> bool {
    schema.contains_key("characters")
}

/// How [`apply`] went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applied {
    pub ok: usize,
    pub failed: usize,
    /// Steps not run because a failure stopped the migration.
    pub not_run: usize,
}

impl Applied {
    /// Every step ran without an error.
    pub fn complete(&self) -> bool {
        self.failed == 0 && self.not_run == 0
    }
}

/// Runs the steps of `plan` on `conn` (the database selected). A failed step is reported to
/// `on_error` (the step's SQL and the error), which answers whether to go on.
pub async fn apply(
    conn: &mut MySqlConnection,
    plan: &Plan,
    mut on_error: impl FnMut(&str, &str) -> bool,
) -> Result<Applied, sqlx::Error> {
    let mut applied = Applied::default();
    for (i, step) in plan.steps.iter().enumerate() {
        match sqlx::raw_sql(AssertSqlSafe(step.sql().to_owned()))
            .execute(&mut *conn)
            .await
        {
            Ok(_) => applied.ok += 1,
            Err(e) => {
                applied.failed += 1;
                if !on_error(step.sql(), &e.to_string()) {
                    applied.not_run = plan.steps.len() - i - 1;
                    break;
                }
            }
        }
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use l2_db_installer::installer::{Schema as Side, load_scripts};

    use super::*;

    fn script(name: &str, sql: &str) -> Script {
        Script {
            schema: Side::Game,
            name: name.to_owned(),
            statements: l2_db_installer::splitter::split_statements(sql),
        }
    }

    const CHARACTERS: &str = "DROP TABLE IF EXISTS `characters`;
CREATE TABLE IF NOT EXISTS `characters` (
  `charId` INT UNSIGNED NOT NULL DEFAULT 0,
  `char_name` VARCHAR(35) NOT NULL,
  `price` DECIMAL(10,2) NOT NULL DEFAULT '0.00',
  `kind` ENUM('a','b,c') DEFAULT 'a',
  PRIMARY KEY (`charId`),
  KEY `char_name` (`char_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO `characters` VALUES (1,'a',0,'a');
REPLACE INTO `characters` VALUES (2,'b',0,'a');
INSERT INTO `characters` VALUES (3,'c',0,'a');";

    fn schema(tables: &[(&str, &[&str])]) -> Schema {
        tables
            .iter()
            .map(|(t, cols)| ((*t).to_owned(), cols.iter().map(|c| (*c).to_owned()).collect()))
            .collect()
    }

    #[test]
    fn a_create_table_gives_its_columns_with_their_definitions() {
        let def = parse_create_table(&l2_db_installer::splitter::split_statements(CHARACTERS)[1]).unwrap();
        assert_eq!(def.name, "characters");
        let names: Vec<&str> = def.columns.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["charId", "char_name", "price", "kind"]);
        assert_eq!(def.columns[2].1, "`price` DECIMAL(10,2) NOT NULL DEFAULT '0.00'");
        assert_eq!(def.columns[3].1, "`kind` ENUM('a','b,c') DEFAULT 'a'");
        assert!(parse_create_table("CREATE INDEX x ON t (a)").is_none());
    }

    #[test]
    fn a_missing_table_gets_its_whole_script() {
        let scripts = [script("characters.sql", CHARACTERS)];
        let plan = plan(&scripts, &Schema::new(), &BTreeSet::new());
        assert_eq!(plan.new_tables, ["characters"]);
        assert_eq!(plan.steps.len(), 5);
        assert!(plan.new_columns.is_empty());
    }

    #[test]
    fn an_existing_table_only_gets_its_missing_columns_and_insert_ignore_rows() {
        let scripts = [script("characters.sql", CHARACTERS)];
        // `price` is missing; names compare without case.
        let existing = schema(&[("characters", &["charid", "char_name", "kind"])]);
        let plan = plan(&scripts, &existing, &BTreeSet::new());
        assert!(plan.new_tables.is_empty());
        assert_eq!(plan.new_columns, [("characters".to_owned(), "price".to_owned())]);
        let sqls: Vec<&str> = plan.steps.iter().map(Step::sql).collect();
        assert_eq!(
            sqls,
            [
                "ALTER TABLE `characters` ADD COLUMN `price` DECIMAL(10,2) NOT NULL DEFAULT '0.00' AFTER `char_name`",
                "INSERT IGNORE INTO `characters` VALUES (1,'a',0,'a');",
            ]
        );
        // Never a DROP, REPLACE or plain INSERT on a table that is there.
        assert!(
            sqls.iter().all(|s| !s.starts_with("DROP")
                && !s.starts_with("REPLACE")
                && !s.starts_with("INSERT INTO"))
        );
        // Complete: nothing but the default rows.
        let existing = schema(&[("characters", &["charid", "char_name", "price", "kind"])]);
        let plan = super::plan(&scripts, &existing, &BTreeSet::new());
        assert_eq!(plan.complete_tables, 1);
        assert!(plan.new_columns.is_empty() && plan.new_tables.is_empty());
        assert_eq!(plan.steps.len(), 1);
        // A table with rows keeps them as they are: no default rows.
        let with_rows: BTreeSet<String> = ["characters".to_owned()].into();
        assert!(super::plan(&scripts, &existing, &with_rows).steps.is_empty());
        // The first column goes FIRST.
        let existing = schema(&[("characters", &["char_name", "price", "kind"])]);
        let plan = super::plan(&scripts, &existing, &with_rows);
        assert_eq!(
            plan.steps[0].sql(),
            "ALTER TABLE `characters` ADD COLUMN `charId` INT UNSIGNED NOT NULL DEFAULT 0 FIRST"
        );
    }

    #[test]
    fn every_shipped_script_has_a_table_and_a_complete_database_drops_nothing() {
        let sql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql");
        let mut scripts = load_scripts(&sql, Side::Login).unwrap();
        scripts.extend(load_scripts(&sql, Side::Game).unwrap());
        assert!(scripts.len() > 100);
        // A database made by the installer: every table with every column.
        let mut full = Schema::new();
        for s in &scripts {
            let def = s
                .statements
                .iter()
                .find_map(|q| {
                    parse_create_table(q).filter(|_| q.to_ascii_uppercase().starts_with("CREATE TABLE"))
                })
                .unwrap_or_else(|| panic!("{} has no table", s.name));
            assert!(!def.columns.is_empty(), "{}", s.name);
            full.insert(
                def.name.to_ascii_lowercase(),
                def.columns.iter().map(|(c, _)| c.to_ascii_lowercase()).collect(),
            );
        }
        let plan = plan(&scripts, &full, &BTreeSet::new());
        assert!(plan.new_tables.is_empty() && plan.new_columns.is_empty());
        assert_eq!(plan.complete_tables, scripts.len());
        for step in &plan.steps {
            assert!(
                step.sql().to_ascii_uppercase().starts_with("INSERT IGNORE"),
                "{step:?}"
            );
        }
        // The empty database gets every script.
        assert_eq!(
            super::plan(&scripts, &Schema::new(), &BTreeSet::new())
                .new_tables
                .len(),
            scripts.len()
        );
        // Every script names its table.
        assert!(scripts.iter().all(|s| script_table(s).is_some()));
    }
}
