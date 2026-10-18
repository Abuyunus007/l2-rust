//! Target `config`: the game server's config classes.
//!
//! Java: gameserver/config/*Config.java and gameserver/config/custom/*Config.java (the classes
//! `ConfigLoader.init()` loads), plus `dist/game/config/**/*.ini` for a cross-check of the keys.
//! Output: crates/l2-config/src/generated/{mod.rs,game/*.rs} and
//! docs/analysis/config-manual-fields.md (the report of everything that is NOT generated).
//!
//! The shape of every generated struct is `crates/l2-config/src/login.rs` (hand-written): one
//! struct per Java class, one field per Java constant, a `/// Key: ..., default ...` doc line and
//! a `from_ini` that calls one `IniFile` getter per field.
//!
//! A statement is "simple" when it is exactly `NAME = reader.getKind("Key", default);` with a
//! trivial default (see `java::eval_default`). Everything else in `load()` is not generated:
//! * Every other declared `public static` field becomes a placeholder field of type `()` with a
//!   `TODO(M2-CFG-01)` doc comment (the Java statements, keys and defaults). The unit type makes
//!   sure nobody reads a wrong value by accident: using the field does not compile until
//!   M2-CFG-01 hand-ports it.
//! * Statements are never dropped. Each one is attached to the fields it mentions (together with
//!   the local variables it uses), or listed as an "orphan" in the report.
//!
//! The generator fails (instead of guessing) on: a class it cannot read, a member that is neither
//! a config field nor a known constant, a getter it has no twin for, a simple-looking statement
//! for a field that is not declared or declared with another type, duplicate names.

mod java;

use super::{Output, render};
use anyhow::{Context, Result, anyhow, bail};
use java::{
    ClassScan, GetterCall, JavaField, Kind, Literal, Statement, eval_default, find_assignment, find_getters,
    identifiers, is_constant_name, parse_getter, parse_string_literal, scan_class, squash,
};
use quote::{ToTokens, format_ident, quote};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

const CONFIG_DIR: &str = "java/org/l2jmobius/gameserver/config";
const CUSTOM_DIR: &str = "java/org/l2jmobius/gameserver/config/custom";
const DIST_CONFIG_DIR: &str = "dist/game/config";
const CUSTOM_IMPORT: &str = "org.l2jmobius.gameserver.config.custom.";

const GENERATED_DIR: &str = "crates/l2-config/src/generated";
const REPORT_PATH: &str = "docs/analysis/config-manual-fields.md";

/// A Java config class as read from disk.
pub struct ClassSource {
    pub name: String,
    /// Relative to the `java/org/l2jmobius/` folder, e.g. `gameserver/config/custom/FooConfig.java`.
    pub java_path: String,
    pub text: String,
}

/// Everything the target reads.
pub struct Sources {
    /// In `ConfigLoader.init()` order.
    pub classes: Vec<ClassSource>,
    /// `.ini` files of `dist/game/config`, keyed by their path below it (`General.ini`,
    /// `Custom/AutoPlay.ini`). Only used for the "keys nobody reads" cross-check.
    pub dist_ini: BTreeMap<String, String>,
}

fn read_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// The class names `ConfigLoader.init()` loads, in order, and which of them are in `custom`.
fn loader_classes(loader: &str) -> Result<Vec<(String, bool)>> {
    let text = java::strip_comments(loader)?;
    let mut out = Vec::new();
    for statement in text.split(';') {
        let s = squash(statement);
        let Some(name) = s.strip_suffix(".load()") else {
            continue;
        };
        let name = name
            .rsplit(|c: char| c.is_whitespace() || c == '{')
            .next()
            .unwrap_or(name);
        if !name.ends_with("Config") || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("ConfigLoader: cannot read the call `{s}`");
        }
        let custom = text.contains(&format!("import {CUSTOM_IMPORT}{name};"));
        out.push((name.to_string(), custom));
    }
    if out.is_empty() {
        bail!("ConfigLoader.init() calls no config class");
    }
    Ok(out)
}

pub fn read_sources(l2j_root: &Path) -> Result<Sources> {
    let main_dir = l2j_root.join(CONFIG_DIR);
    let custom_dir = l2j_root.join(CUSTOM_DIR);
    let loader = read_file(&main_dir.join("ConfigLoader.java"))?;
    let listed = loader_classes(&loader)?;

    // No config class may be forgotten: every *Config.java on disk must be loaded by the loader.
    let on_disk = |dir: &Path| -> Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
            let file = entry?.file_name().to_string_lossy().into_owned();
            if let Some(stem) = file.strip_suffix(".java") {
                names.insert(stem.to_string());
            }
        }
        Ok(names)
    };
    let mut expected = on_disk(&main_dir)?;
    expected.remove("ConfigLoader");
    expected.extend(on_disk(&custom_dir)?);
    let loaded: BTreeSet<_> = listed.iter().map(|(n, _)| n.clone()).collect();
    if expected != loaded {
        bail!(
            "ConfigLoader and the config folders disagree: only on disk {:?}, only loaded {:?}",
            expected.difference(&loaded).collect::<Vec<_>>(),
            loaded.difference(&expected).collect::<Vec<_>>()
        );
    }

    let mut classes = Vec::new();
    for (name, custom) in listed {
        let (dir, rel) = if custom {
            (&custom_dir, format!("gameserver/config/custom/{name}.java"))
        } else {
            (&main_dir, format!("gameserver/config/{name}.java"))
        };
        classes.push(ClassSource {
            text: read_file(&dir.join(format!("{name}.java")))?,
            name,
            java_path: rel,
        });
    }

    let mut dist_ini = BTreeMap::new();
    let dist = l2j_root.join(DIST_CONFIG_DIR);
    for (sub, prefix) in [(dist.clone(), ""), (dist.join("Custom"), "Custom/")] {
        let Ok(entries) = std::fs::read_dir(&sub) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.ends_with(".ini") {
                dist_ini.insert(format!("{prefix}{file}"), read_file(&entry.path())?);
            }
        }
    }
    Ok(Sources { classes, dist_ini })
}

// ---------------------------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------------------------

struct SimpleField {
    name: String,
    /// The getter that reads it.
    kind: Kind,
    /// The Java type of the field; differs from `kind` after an implicit widening.
    declared: Kind,
    key: String,
    default: Literal,
}

struct ManualField {
    java: JavaField,
    /// Indexes into `ClassModel::others`, ascending.
    statements: Vec<usize>,
    getters: Vec<GetterCall>,
    /// Lines (outside `load()`) that mention the field, when `load()` never does.
    elsewhere: Vec<usize>,
}

enum Field {
    Simple(SimpleField),
    Manual(ManualField),
}

impl Field {
    fn name(&self) -> &str {
        match self {
            Self::Simple(s) => &s.name,
            Self::Manual(m) => &m.java.name,
        }
    }
}

struct ClassModel {
    java_name: String,
    java_path: String,
    /// `general`, `geo_engine`, ...
    module: String,
    /// Path below the config directory: `General.ini`, `Custom/AutoPlay.ini`.
    ini_file: String,
    fields: Vec<Field>,
    /// The statements of `load()` that are not simple (and not a reader declaration).
    others: Vec<Statement>,
    /// Indexes into `others` that no field claimed.
    orphans: Vec<usize>,
    /// Simple fields that a non-simple statement mentions: (field, index into `others`).
    touched: Vec<(String, usize)>,
    /// Indexes into `others` that look simple but have a default we cannot translate.
    nontrivial: Vec<usize>,
    /// Keys of the dist `.ini` that no getter call with a literal key reads.
    unread_keys: Vec<String>,
}

impl ClassModel {
    fn simple_count(&self) -> usize {
        self.fields
            .iter()
            .filter(|f| matches!(f, Field::Simple(_)))
            .count()
    }

    fn manual(&self) -> impl Iterator<Item = &ManualField> {
        self.fields.iter().filter_map(|f| match f {
            Field::Manual(m) => Some(m),
            Field::Simple(_) => None,
        })
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
    "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield", "try", "gen",
];

/// `GEODATA_PATH` -> `geodata_path`.
fn rust_field_name(java_name: &str) -> String {
    let lower = java_name.to_ascii_lowercase();
    if RUST_KEYWORDS.contains(&lower.as_str()) {
        format!("{lower}_")
    } else {
        lower
    }
}

/// `GeoEngineConfig` -> `geo_engine`.
fn module_name(class_name: &str) -> Result<String> {
    let base = class_name
        .strip_suffix("Config")
        .filter(|b| !b.is_empty())
        .ok_or_else(|| anyhow!("{class_name}: class name does not end in `Config`"))?;
    let chars: Vec<char> = base.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let acronym_end = i > 0
                && chars[i - 1].is_ascii_uppercase()
                && chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if prev_lower || acronym_end {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            bail!("{class_name}: unexpected character `{c}` in the class name");
        }
    }
    if RUST_KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    Ok(out)
}

/// `new ConfigReader(ARG)` declarations: `final ConfigReader config = new ConfigReader(X_FILE);`.
fn parse_reader_decl(stmt: &str) -> Option<(String, String)> {
    let s = stmt.trim().strip_prefix("final ").unwrap_or(stmt).trim_start();
    let s = s.strip_prefix("ConfigReader ")?.trim_start();
    let (var, rest) = s.split_once('=')?;
    let arg = rest
        .trim()
        .strip_prefix("new ConfigReader(")?
        .strip_suffix(");")?
        .trim()
        .to_string();
    Some((var.trim().to_string(), arg))
}

/// Resolves the argument of `new ConfigReader(..)` to a path below the config directory.
fn resolve_ini_file(class: &ClassScan, arg: &str) -> Result<String> {
    let path = match parse_string_literal(arg) {
        Some((literal, rest)) if rest.trim().is_empty() => literal,
        _ => class
            .constants
            .get(arg)
            .cloned()
            .ok_or_else(|| anyhow!("{}: ConfigReader({arg}) is not a known file constant", class.name))?,
    };
    // RebirthConfig writes `config/Custom/Rebirth.ini` without the `./`.
    path.strip_prefix("./config/")
        .or_else(|| path.strip_prefix("config/"))
        .filter(|rest| rest.ends_with(".ini"))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{}: `{path}` is not a (./)config/*.ini path", class.name))
}

/// If `stmt` is `NAME = reader.getKind("Key", default);` returns the pieces. `Ok(None)` means
/// "not that shape" (chained call, arithmetic, other receiver, no reader at all).
fn parse_simple_shape<'a>(stmt: &'a str, readers: &[String]) -> Result<Option<(&'a str, GetterCall)>> {
    let Some(eq) = find_assignment(stmt) else {
        return Ok(None);
    };
    let name = stmt[..eq].trim();
    if !is_constant_name(name) {
        return Ok(None);
    }
    let rhs = eq + 1 + (stmt[eq + 1..].len() - stmt[eq + 1..].trim_start().len());
    let Some(call) = parse_getter(stmt, rhs) else {
        return Ok(None);
    };
    if !readers.contains(&call.reader) || stmt[call.end..].trim() != ";" {
        return Ok(None);
    }
    if Kind::from_getter(&call.kind).is_none() {
        bail!(
            "`{}`: no IniFile twin for the getter `get{}`",
            squash(stmt),
            call.kind
        );
    }
    if !call.key_is_literal || call.default_expr.is_none() {
        bail!("`{}`: getter without a literal key and a default", squash(stmt));
    }
    Ok(Some((name, call)))
}

/// Local variables (lower-case names) that `stmt` assigns anywhere inside, e.g. `split` in
/// `final String[] split = ...` and `charNamePattern` in `try { charNamePattern = ...; } ...`.
fn assigned_locals(stmt: &str) -> Vec<String> {
    let b = stmt.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = java::skip_literal(b, i),
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let name = &stmt[start..i];
                let after = stmt[i..].trim_start();
                let before = stmt[..start].trim_end();
                let assigned = after.starts_with('=') && !after.starts_with("==");
                if assigned
                    && !before.ends_with('.')
                    && name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
                    && !out.iter().any(|o| o == name)
                {
                    out.push(name.to_string());
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// `Pattern charNamePattern;` -> `charNamePattern`: a local declared without a value.
fn bare_declaration(stmt: &str) -> Option<String> {
    let body = stmt.trim().strip_suffix(';')?.trim();
    let words: Vec<&str> = body.split_whitespace().collect();
    let name = words.last()?;
    let plain = body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || " _<>[],".contains(c));
    let valid = name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_');
    (plain && valid && (2..=3).contains(&words.len())).then(|| (*name).to_string())
}

fn analyze(source: &ClassSource, dist_ini: &BTreeMap<String, String>) -> Result<ClassModel> {
    let scan = scan_class(&source.text).with_context(|| format!("scanning {}", source.name))?;
    if scan.name != source.name {
        bail!("{} declares class {}", source.java_path, scan.name);
    }
    let name = &scan.name;
    let ctx = |what: &str| format!("{name}: {what}");

    // Readers declared in load(): the first one is the class's file.
    let mut reader_vars = Vec::new();
    let mut ini_file = None;
    for stmt in &scan.load {
        if let Some((var, arg)) = parse_reader_decl(&stmt.text) {
            if ini_file.is_none() {
                ini_file = Some(resolve_ini_file(&scan, &arg)?);
            }
            reader_vars.push(var);
        }
    }
    let ini_file = ini_file.ok_or_else(|| anyhow!(ctx("load() declares no ConfigReader")))?;
    let primary = reader_vars[0].clone();

    // Classify the statements of load().
    let mut simple: HashMap<String, SimpleField> = HashMap::new();
    let mut others: Vec<Statement> = Vec::new();
    let mut nontrivial = Vec::new();
    for stmt in &scan.load {
        if parse_reader_decl(&stmt.text).is_some() {
            continue;
        }
        match parse_simple_shape(&stmt.text, &reader_vars)
            .with_context(|| ctx(&format!("line {}", stmt.line)))?
        {
            Some((field_name, call)) => {
                if call.reader != primary {
                    bail!(
                        "{}: line {}: simple statement reads another file",
                        name,
                        stmt.line
                    );
                }
                let declared = scan.fields.iter().find(|f| f.name == field_name).ok_or_else(|| {
                    anyhow!(ctx(&format!(
                        "line {}: `{field_name}` is not a declared field",
                        stmt.line
                    )))
                })?;
                let kind = Kind::from_getter(&call.kind).ok_or_else(|| anyhow!("unreachable getter kind"))?;
                // Java widens implicitly (`double X = config.getInt(..)`); we only follow the
                // widenings Rust has a lossless `From` for.
                let Some(declared_kind) =
                    Kind::from_java_type(&declared.java_type).filter(|d| kind.converts_to(*d))
                else {
                    bail!(
                        "{}: line {}: `{field_name}` is declared `{}` but read with get{}",
                        name,
                        stmt.line,
                        declared.java_type,
                        call.kind
                    );
                };
                let default_expr = call.default_expr.as_deref().unwrap_or_default();
                match eval_default(kind, default_expr) {
                    Some(default) => {
                        if simple
                            .insert(
                                field_name.to_string(),
                                SimpleField {
                                    name: field_name.to_string(),
                                    kind,
                                    declared: declared_kind,
                                    key: call.key.clone(),
                                    default,
                                },
                            )
                            .is_some()
                        {
                            bail!("{}: line {}: `{field_name}` is assigned twice", name, stmt.line);
                        }
                    }
                    None => {
                        nontrivial.push(others.len());
                        others.push(stmt.clone());
                    }
                }
            }
            None => others.push(stmt.clone()),
        }
    }

    // Which statements belong to which field.
    let declared: BTreeSet<&str> = scan.fields.iter().map(|f| f.name.as_str()).collect();
    let mut claims: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut claimed = vec![false; others.len()];
    let mut touched = Vec::new();
    let mut locals: HashMap<String, usize> = HashMap::new();
    // `Pattern x;` lines: they carry no behaviour, but belong to whoever uses `x`.
    let mut declarations: HashMap<String, Vec<usize>> = HashMap::new();
    let mut uses: Vec<BTreeSet<String>> = Vec::new();
    for (k, stmt) in others.iter().enumerate() {
        let idents = identifiers(&stmt.text);
        let own: BTreeSet<&str> = idents
            .iter()
            .filter(|(id, prev)| *prev != Some(b'.') && declared.contains(id))
            .map(|(id, _)| *id)
            .collect();
        let used_locals: BTreeSet<String> = idents
            .iter()
            .filter(|(id, prev)| *prev != Some(b'.') && locals.contains_key(*id))
            .map(|(id, _)| (*id).to_string())
            .collect();
        for field in own {
            if simple.contains_key(field) {
                touched.push((field.to_string(), k));
                claimed[k] = true;
            } else {
                let set = claims.entry(field.to_string()).or_default();
                set.insert(k);
                // The definitions of the locals this statement uses (and theirs, recursively).
                for local in &used_locals {
                    set.extend(declarations.get(local).into_iter().flatten().copied());
                }
                let mut stack: Vec<usize> = used_locals.iter().map(|l| locals[l]).collect();
                while let Some(def) = stack.pop() {
                    if set.insert(def) {
                        stack.extend(
                            uses[def]
                                .iter()
                                .filter_map(|l| locals.get(l).copied().filter(|&d| d < def)),
                        );
                    }
                }
                claimed[k] = true;
            }
        }
        for local in assigned_locals(&stmt.text) {
            locals.insert(local, k);
        }
        if let Some(local) = bare_declaration(&stmt.text) {
            declarations.entry(local).or_default().push(k);
        }
        uses.push(used_locals);
    }
    for set in claims.values() {
        for &k in set {
            claimed[k] = true;
        }
    }
    let orphans: Vec<usize> = (0..others.len()).filter(|&k| !claimed[k]).collect();

    // The fields, in declaration order.
    let mut fields = Vec::new();
    for java in &scan.fields {
        if let Some(s) = simple.remove(&java.name) {
            fields.push(Field::Simple(s));
            continue;
        }
        let statements: Vec<usize> = claims
            .get(&java.name)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        let getters = statements
            .iter()
            .flat_map(|&k| find_getters(&others[k].text, &reader_vars))
            .collect();
        let elsewhere = if statements.is_empty() {
            mentions_outside_load(&scan, java)
        } else {
            Vec::new()
        };
        fields.push(Field::Manual(ManualField {
            java: java.clone(),
            statements,
            getters,
            elsewhere,
        }));
    }
    if let Some(left) = simple.keys().next() {
        bail!(ctx(&format!("simple field `{left}` was lost")));
    }

    // Keys of the dist file that no getter with a literal key reads.
    let mut known: BTreeSet<String> = find_getters(&scan.text, &reader_vars)
        .into_iter()
        .filter(|g| g.key_is_literal)
        .map(|g| g.key)
        .collect();
    known.extend(fields.iter().filter_map(|f| match f {
        Field::Simple(s) => Some(s.key.clone()),
        Field::Manual(_) => None,
    }));
    let unread_keys = dist_ini
        .get(&ini_file)
        .map(|text| {
            ini_keys(text)
                .into_iter()
                .filter(|k| !known.contains(k))
                .collect()
        })
        .unwrap_or_default();

    Ok(ClassModel {
        java_name: scan.name.clone(),
        java_path: source.java_path.clone(),
        module: module_name(&scan.name)?,
        ini_file,
        fields,
        others,
        orphans,
        touched,
        nontrivial,
        unread_keys,
    })
}

/// Lines outside `load()` (other than the declaration) where `field` is used.
fn mentions_outside_load(scan: &ClassScan, field: &JavaField) -> Vec<usize> {
    let (from, to) = scan.load_range;
    let mut lines = Vec::new();
    let mut offset = 0;
    for (index, line) in scan.text.split('\n').enumerate() {
        let inside_load = offset >= from && offset < to;
        let mentioned = identifiers(line)
            .iter()
            .any(|(id, prev)| *id == field.name && *prev != Some(b'.'));
        if !inside_load && mentioned && index + 1 != field.line {
            lines.push(index + 1);
        }
        offset += line.len() + 1;
    }
    lines
}

/// Keys of a Java `.properties` style file (comment lines skipped; first `=`/`:`/blank splits).
fn ini_keys(text: &str) -> Vec<String> {
    let mut keys = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with(['#', '!']) {
            continue;
        }
        let end = line.find(['=', ':', ' ', '\t']).unwrap_or(line.len());
        keys.insert(line[..end].to_string());
    }
    keys.into_iter().collect()
}

// ---------------------------------------------------------------------------------------------
// Rust output
// ---------------------------------------------------------------------------------------------

/// A statement as one line for a doc comment (backticks would end the code span).
fn doc_snippet(stmt: &Statement, limit: usize) -> String {
    let mut text = squash(&stmt.text).replace('`', "'");
    if text.len() > limit {
        let cut = (0..=limit).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
        text.truncate(cut);
        text.push_str(" ...");
    }
    text
}

fn literal_doc(lit: &Literal) -> String {
    match lit {
        Literal::Bool(b) => b.to_string(),
        Literal::Int(i) => i.to_string(),
        Literal::Float(f) => f.clone(),
        Literal::Str(s) if s.is_empty() => "empty".to_string(),
        Literal::Str(s) => format!("`{}`", s.replace('`', "'").replace('\n', "\\n")),
    }
}

fn literal_expr(lit: &Literal) -> Result<syn::Expr> {
    Ok(match lit {
        Literal::Bool(b) => syn::parse_str(&b.to_string())?,
        Literal::Int(i) => syn::parse_str(&i.to_string())?,
        Literal::Float(f) => syn::parse_str(f)?,
        Literal::Str(s) => syn::parse_quote!(#s),
    })
}

fn class_tokens(class: &ClassModel) -> Result<impl ToTokens> {
    let struct_name = format_ident!("{}", class.java_name);
    let mut seen = BTreeSet::new();
    let mut members = Vec::new();
    let mut inits = Vec::new();
    for field in &class.fields {
        let rust_name = rust_field_name(field.name());
        if !seen.insert(rust_name.clone()) {
            bail!("{}: two fields become `{rust_name}`", class.java_name);
        }
        let ident = format_ident!("{}", rust_name);
        match field {
            Field::Simple(s) => {
                let ty: syn::Type = syn::parse_str(s.declared.rust_type())?;
                let mut docs = vec![format!(" Key: `{}`, default {}.", s.key, literal_doc(&s.default))];
                // Java code that uses the field after reading it: clamping, defaults, unit changes.
                let touching: BTreeSet<usize> = class
                    .touched
                    .iter()
                    .filter(|(name, _)| *name == s.name)
                    .map(|(_, k)| *k)
                    .collect();
                for &k in touching.iter().take(2) {
                    docs.push(format!(
                        " TODO(M2-CFG-01): Java also uses this field after reading it (line {}): `{}`",
                        class.others[k].line,
                        doc_snippet(&class.others[k], 160)
                    ));
                }
                let docs = docs.iter().map(|d| quote! { #[doc = #d] });
                members.push(quote! { #(#docs)* pub #ident: #ty, });
                let getter = format_ident!("{}", s.kind.rust_getter());
                let key = &s.key;
                let default = literal_expr(&s.default)?;
                inits.push(if s.kind == s.declared {
                    quote! { #ident: ini.#getter(#key, #default), }
                } else {
                    let ty: syn::Type = syn::parse_str(s.declared.rust_type())?;
                    quote! { #ident: #ty::from(ini.#getter(#key, #default)), }
                });
            }
            Field::Manual(m) => {
                let mut docs = vec![format!(
                    " TODO(M2-CFG-01): not generated, hand-port it. Java type `{}`; the unit type makes any use fail to compile until then.",
                    m.java.java_type.replace('`', "'")
                )];
                match m.statements.first() {
                    Some(&first) => {
                        let line = class.others[first].line;
                        docs.push(format!(
                            " Java (line {line}): `{}`",
                            doc_snippet(&class.others[first], 200)
                        ));
                        if m.statements.len() > 1 {
                            docs.push(format!(
                                " ... and {} more statement(s), see docs/analysis/config-manual-fields.md.",
                                m.statements.len() - 1
                            ));
                        }
                    }
                    None => docs.push(format!(
                        " Java: `load()` never assigns it{}.",
                        if m.elsewhere.is_empty() {
                            String::new()
                        } else {
                            format!(" (used on line(s) {:?} of the class)", m.elsewhere)
                        }
                    )),
                }
                for g in m.getters.iter().take(4) {
                    if g.key_is_literal {
                        docs.push(format!(
                            " Key: `{}`, Java default `{}` (before the Java post-processing).",
                            g.key,
                            squash(g.default_expr.as_deref().unwrap_or("-")).replace('`', "'")
                        ));
                    }
                }
                let docs = docs.iter().map(|d| quote! { #[doc = #d] });
                members.push(quote! { #(#docs)* pub #ident: (), });
                inits.push(quote! { #ident: (), });
            }
        }
    }
    let file = class.ini_file.as_str();
    let struct_doc = format!(
        " Settings of `config/{}` (Java: `{}`).",
        class.ini_file, class.java_name
    );
    let ini_param = if class.simple_count() == 0 {
        format_ident!("_ini")
    } else {
        format_ident!("ini")
    };
    let from_doc = format!(" Reads the values like `{}.load()`.", class.java_name);
    Ok(quote! {
        use crate::IniFile;

        #[doc = #struct_doc]
        #[derive(Debug, Clone, PartialEq)]
        pub struct #struct_name {
            #(#members)*
        }

        impl #struct_name {
            /// File name inside the config directory.
            pub const FILE: &'static str = #file;

            #[doc = #from_doc]
            pub fn from_ini(#ini_param: &IniFile) -> Self {
                Self {
                    #(#inits)*
                }
            }
        }

        impl Default for #struct_name {
            /// Every key missing: the Java defaults.
            fn default() -> Self {
                Self::from_ini(&IniFile::empty(Self::FILE))
            }
        }
    })
}

fn game_mod_tokens(classes: &[ClassModel]) -> impl ToTokens {
    let modules = classes.iter().map(|c| format_ident!("{}", c.module));
    let reexports = classes.iter().map(|c| {
        let module = format_ident!("{}", c.module);
        let name = format_ident!("{}", c.java_name);
        quote! { pub use #module::#name; }
    });
    let table = classes.iter().map(|c| {
        let name = &c.java_name;
        let file = &c.ini_file;
        quote! { (#name, #file) }
    });
    quote! {
        #(#[rustfmt::skip] pub mod #modules;)*
        #(#reexports)*

        /// Every generated class with its `.ini` file (below the config directory), in the order
        /// of Java's `ConfigLoader.init()`.
        pub const CLASSES: &[(&str, &str)] = &[#(#table),*];
    }
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

fn report(classes: &[ClassModel]) -> String {
    let mut out = String::new();
    let total_simple: usize = classes.iter().map(ClassModel::simple_count).sum();
    let total_manual: usize = classes.iter().map(|c| c.manual().count()).sum();
    let _ = writeln!(out, "<!-- @generated by codegen — do not edit -->");
    let _ = writeln!(out, "# Game config: fields left for hand-porting (M2-CFG-01)\n");
    let _ = writeln!(
        out,
        "Generated by `cargo run -p codegen -- config` from the Java classes listed in `ConfigLoader.init()`.\n\
         Every declared `public static` field of those classes is either generated (a plain\n\
         `NAME = config.getX(\"Key\", default);` line) or listed here. In the Rust structs the\n\
         non-generated fields are `()` placeholders whose doc comment starts with `TODO(M2-CFG-01)`.\n"
    );
    let _ = writeln!(
        out,
        "{} classes, {total_simple} generated fields, {total_manual} fields to hand-port.\n",
        classes.len()
    );
    let _ = writeln!(
        out,
        "| Java class | .ini file | generated | hand-port | orphan statements |"
    );
    let _ = writeln!(out, "|---|---|---:|---:|---:|");
    for c in classes {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {} | {} |",
            c.java_name,
            c.ini_file,
            c.simple_count(),
            c.manual().count(),
            c.orphans.len()
        );
    }
    let _ = writeln!(out);
    for c in classes {
        let nothing = c.manual().next().is_none()
            && c.orphans.is_empty()
            && c.touched.is_empty()
            && c.nontrivial.is_empty()
            && c.unread_keys.is_empty();
        if nothing {
            continue;
        }
        let _ = writeln!(out, "## {}\n", c.java_name);
        let _ = writeln!(out, "Java: `{}`, file `{}`.\n", c.java_path, c.ini_file);
        if c.manual().next().is_some() {
            let _ = writeln!(out, "### Fields to hand-port\n");
            for m in c.manual() {
                let _ = writeln!(
                    out,
                    "- `{}` (`{}`, line {})",
                    m.java.name, m.java.java_type, m.java.line
                );
                let keys: Vec<String> = m
                    .getters
                    .iter()
                    .map(|g| {
                        format!(
                            "`{}` (default `{}`)",
                            g.key.replace('`', "'"),
                            squash(g.default_expr.as_deref().unwrap_or("-")).replace('`', "'")
                        )
                    })
                    .collect();
                if !keys.is_empty() {
                    let _ = writeln!(out, "  - keys: {}", keys.join(", "));
                }
                if let Some(init) = &m.java.init {
                    let _ = writeln!(out, "  - initializer: `{}`", squash(init).replace('`', "'"));
                }
                for &k in &m.statements {
                    let s = &c.others[k];
                    let _ = writeln!(out, "  - line {}: `{}`", s.line, doc_snippet(s, 400));
                }
                if m.statements.is_empty() {
                    let _ = writeln!(
                        out,
                        "  - `load()` never assigns it; other lines using it: {:?}",
                        m.elsewhere
                    );
                }
            }
            let _ = writeln!(out);
        }
        if !c.touched.is_empty() {
            let _ = writeln!(
                out,
                "### Generated fields that other statements also use\n\n\
                 The generated value is the raw getter result. If one of these statements changes the\n\
                 value afterwards (clamping, unit conversion), port that by hand.\n"
            );
            for (field, k) in &c.touched {
                let s = &c.others[*k];
                let _ = writeln!(out, "- `{field}`: line {}: `{}`", s.line, doc_snippet(s, 400));
            }
            let _ = writeln!(out);
        }
        if !c.nontrivial.is_empty() {
            let _ = writeln!(
                out,
                "### Plain getter lines with a default we could not translate\n"
            );
            for &k in &c.nontrivial {
                let s = &c.others[k];
                let _ = writeln!(out, "- line {}: `{}`", s.line, doc_snippet(s, 400));
            }
            let _ = writeln!(out);
        }
        if !c.orphans.is_empty() {
            let _ = writeln!(
                out,
                "### Statements of `load()` that belong to no field of this class\n\n\
                 Side effects on other classes, logging, calls into helpers. Not generated.\n"
            );
            for &k in &c.orphans {
                let s = &c.others[k];
                let _ = writeln!(out, "- line {}: `{}`", s.line, doc_snippet(s, 400));
            }
            let _ = writeln!(out);
        }
        if !c.unread_keys.is_empty() {
            let _ = writeln!(
                out,
                "### Keys in the dist `.ini` that no getter with a literal key reads\n\n\
                 Read through a computed key (see the helper methods in the Java class), or unused.\n"
            );
            let _ = writeln!(
                out,
                "{}\n",
                c.unread_keys
                    .iter()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// Reads and classifies every class (used by the tests, which want the numbers).
fn analyze_all(sources: &Sources) -> Result<Vec<ClassModel>> {
    let mut classes: Vec<ClassModel> = Vec::new();
    for source in &sources.classes {
        let class = analyze(source, &sources.dist_ini)?;
        if let Some(other) = classes
            .iter()
            .find(|c| c.module == class.module || c.java_name == class.java_name)
        {
            bail!(
                "{} and {} map to the same Rust module",
                other.java_name,
                class.java_name
            );
        }
        classes.push(class);
    }
    Ok(classes)
}

pub fn generate(sources: &Sources) -> Result<Vec<Output>> {
    let classes = analyze_all(sources)?;
    let mut outputs = Vec::new();
    for class in &classes {
        let note = format!(
            "// Java: org/l2jmobius/{}\n// Regenerate: cargo run -p codegen -- config",
            class.java_path
        );
        outputs.push(Output {
            path: format!("{GENERATED_DIR}/game/{}.rs", class.module).into(),
            contents: render(&note, class_tokens(class)?)?,
        });
    }
    outputs.push(Output {
        path: format!("{GENERATED_DIR}/game/mod.rs").into(),
        contents: render(
            "// Source: Java gameserver/config/**/*Config.java (see codegen `config` target)\n// Regenerate: cargo run -p codegen -- config",
            game_mod_tokens(&classes),
        )?,
    });
    outputs.push(Output {
        path: format!("{GENERATED_DIR}/mod.rs").into(),
        contents: render(
            "// Lists every generated module of this crate.",
            quote! { #[rustfmt::skip] pub mod game; },
        )?,
    });
    outputs.push(Output {
        path: REPORT_PATH.into(),
        contents: report(&classes),
    });
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_source(name: &str, body: &str) -> ClassSource {
        ClassSource {
            name: name.to_string(),
            java_path: format!("gameserver/config/{name}.java"),
            text: format!(
                "public class {name}\n{{\n\tprivate static final String F = \"./config/Foo.ini\";\n{body}\n}}\n"
            ),
        }
    }

    const BODY: &str = r#"
	public static boolean ENABLED;
	public static int LIMIT;
	public static long BIG;
	public static float RATE;
	public static double FACTOR;
	public static String NAME;
	public static byte LEVEL;
	public static Set<Integer> IDS;
	public static int MILLIS;
	public static int[] PAIR;
	public static int NEVER_SET;
	public static void load()
	{
		final ConfigReader config = new ConfigReader(F);
		ENABLED = config.getBoolean("Enabled", true);
		LIMIT = config.getInt("Limit", Integer.MAX_VALUE);
		BIG = config.getLong("Big", 50000000000L);
		RATE = config.getFloat("Rate", 0.6f);
		FACTOR = config.getDouble("Factor", 100);
		NAME = config.getString("Name", "a \"b\"");
		LEVEL = config.getByte("Level", (byte) 20);
		final String[] split = config.getString("Ids", "1,2").split(",");
		IDS = new HashSet<>(split.length);
		for (String id : split)
		{
			IDS.add(Integer.parseInt(id));
		}
		MILLIS = config.getInt("Seconds", 60) * 1000;
		PAIR = new int[] { config.getInt("A", 1), 2 };
		LIMIT = Math.max(LIMIT, 1);
		OtherConfig.THING = false;
	}
"#;

    #[test]
    fn classifies_statements() {
        let sources = Sources {
            classes: vec![class_source("FooConfig", BODY)],
            dist_ini: BTreeMap::from([(
                "Foo.ini".to_string(),
                "# c\nEnabled = true\nStray = 1\nIds=1\n".to_string(),
            )]),
        };
        let classes = analyze_all(&sources).unwrap();
        let c = &classes[0];
        assert_eq!(c.ini_file, "Foo.ini");
        assert_eq!(c.module, "foo");
        assert_eq!(c.simple_count(), 7);
        let manual: Vec<_> = c
            .manual()
            .map(|m| (m.java.name.as_str(), m.statements.len()))
            .collect();
        // IDS: feeder + assignment + loop; MILLIS and PAIR: one statement; NEVER_SET: none.
        assert_eq!(manual, [("IDS", 3), ("MILLIS", 1), ("PAIR", 1), ("NEVER_SET", 0)]);
        assert_eq!(c.touched.len(), 1);
        assert_eq!(c.touched[0].0, "LIMIT");
        assert_eq!(c.orphans.len(), 1);
        assert!(c.others[c.orphans[0]].text.starts_with("OtherConfig.THING"));
        // `Stray` is in the ini but nothing reads it; `Ids`, `A` etc. are read.
        assert_eq!(c.unread_keys, ["Stray"]);
    }

    #[test]
    fn generates_compilable_looking_rust() {
        let sources = Sources {
            classes: vec![class_source("FooConfig", BODY)],
            dist_ini: BTreeMap::new(),
        };
        let outputs = generate(&sources).unwrap();
        let paths: Vec<_> = outputs
            .iter()
            .map(|o| o.path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            [
                "crates/l2-config/src/generated/game/foo.rs",
                "crates/l2-config/src/generated/game/mod.rs",
                "crates/l2-config/src/generated/mod.rs",
                "docs/analysis/config-manual-fields.md",
            ]
        );
        let foo = &outputs[0].contents;
        assert!(foo.starts_with(super::super::HEADER));
        for expected in [
            "pub enabled: bool,",
            "pub big: i64,",
            "pub level: i8,",
            "enabled: ini.get_bool(\"Enabled\", true),",
            "limit: ini.get_int(\"Limit\", 2147483647),",
            "big: ini.get_long(\"Big\", 50000000000),",
            "rate: ini.get_float(\"Rate\", 0.6),",
            "factor: ini.get_double(\"Factor\", 100.0),",
            "name: ini.get_string(\"Name\", \"a \\\"b\\\"\"),",
            "level: ini.get_byte(\"Level\", 20),",
            "pub ids: (),",
            "pub millis: (),",
            "TODO(M2-CFG-01)",
            "Key: `Seconds`, Java default `60`",
            "pub const FILE: &'static str = \"Foo.ini\";",
        ] {
            assert!(foo.contains(expected), "missing `{expected}` in:\n{foo}");
        }
        // The output must be valid Rust.
        syn::parse_file(foo).unwrap();
        let report = &outputs[3].contents;
        assert!(report.contains("`MILLIS`"));
        assert!(report.contains("OtherConfig.THING"));
    }

    #[test]
    fn fails_loudly() {
        let run = |body: &str| analyze(&class_source("FooConfig", body), &BTreeMap::new());
        let head = "public static int A;\npublic static void load()\n{\nfinal ConfigReader config = new ConfigReader(F);\n";
        // Unknown getter in a plain statement.
        let err = run(&format!("{head}A = config.getDuration(\"A\", 1);\n}}"))
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("no IniFile twin"), "{err:#}");
        // Undeclared field.
        let err = run(&format!("{head}B = config.getInt(\"A\", 1);\n}}"))
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("not a declared field"), "{err:#}");
        // Wrong declared type.
        let err = run(&format!("{head}A = config.getLong(\"A\", 1);\n}}"))
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("declared `int`"), "{err:#}");
        // Assigned twice.
        let err = run(&format!(
            "{head}A = config.getInt(\"A\", 1);\nA = config.getInt(\"A\", 2);\n}}"
        ))
        .err()
        .unwrap();
        assert!(format!("{err:#}").contains("assigned twice"), "{err:#}");
        // No reader.
        assert!(run("public static int A;\npublic static void load()\n{\nA = 1;\n}").is_err());
        // Unknown member.
        assert!(run("protected int q;\npublic static void load()\n{\n}").is_err());
        // A default we cannot translate is not an error: the field goes to the manual list.
        let class = run(&format!("{head}A = config.getInt(\"A\", SOME_CONSTANT);\n}}")).unwrap();
        assert_eq!(class.simple_count(), 0);
        assert_eq!(class.nontrivial.len(), 1);
        assert_eq!(class.manual().next().unwrap().java.name, "A");
    }

    /// Runs only where the Java checkout exists (not in CI): the numbers reported for M2-GEN-02.
    #[test]
    fn real_java_counts() {
        let root = std::path::PathBuf::from(
            std::env::var("L2J_ROOT").unwrap_or_else(|_| r"C:\l2-rust\l2jmobius".to_string()),
        );
        if !root.is_dir() {
            eprintln!("skipped: no Java source at {}", root.display());
            return;
        }
        let classes = analyze_all(&read_sources(&root).unwrap()).unwrap();
        let simple: usize = classes.iter().map(ClassModel::simple_count).sum();
        let manual: usize = classes.iter().map(|c| c.manual().count()).sum();
        let declared: usize = classes.iter().map(|c| c.fields.len()).sum();
        assert_eq!(classes.len(), 60);
        assert_eq!(
            classes
                .iter()
                .filter(|c| c.ini_file.starts_with("Custom/"))
                .count(),
            44
        );
        assert_eq!(simple, 1024);
        assert_eq!(manual, 185);
        assert_eq!(simple + manual, declared);
        // Every simple field has a key that exists in its dist file or is simply defaulted; the
        // classes that read keys through helper methods are the ones with unread keys.
        let helper_classes: Vec<_> = classes
            .iter()
            .filter(|c| !c.unread_keys.is_empty())
            .map(|c| c.java_name.as_str())
            .collect();
        assert!(helper_classes.contains(&"FloodProtectorConfig"));
    }

    #[test]
    fn names() {
        assert_eq!(module_name("GeneralConfig").unwrap(), "general");
        assert_eq!(module_name("GeoEngineConfig").unwrap(), "geo_engine");
        assert_eq!(module_name("FindPvpConfig").unwrap(), "find_pvp");
        assert_eq!(
            module_name("NpcStatMultipliersConfig").unwrap(),
            "npc_stat_multipliers"
        );
        assert!(module_name("Config").is_err());
        assert_eq!(
            rust_field_name("ALT_LOTTERY_2_AND_1_NUMBER_PRIZE"),
            "alt_lottery_2_and_1_number_prize"
        );
        assert_eq!(rust_field_name("TYPE"), "type_");
        assert_eq!(ini_keys("# x\n  A = 1\nB:2\nC 3\n! y\n"), ["A", "B", "C"]);
    }

    #[test]
    fn loader_class_list() {
        let loader = "import org.l2jmobius.gameserver.config.custom.FooConfig;\n// X.load();\npublic class ConfigLoader { public static void init() { ServerConfig.load(); // c\n FooConfig.load(); } }";
        assert_eq!(
            loader_classes(loader).unwrap(),
            [
                ("ServerConfig".to_string(), false),
                ("FooConfig".to_string(), true)
            ]
        );
    }
}
