//! A small scanner for the shape of the L2JMobius config classes.
//!
//! Like [`crate::java_enum`] this is deliberately not a Java parser. It understands exactly what
//! the `gameserver/config/**` classes are made of and fails loudly on anything else:
//!
//! ```text
//! public class FooConfig
//! {
//!     private static final String FOO_CONFIG_FILE = "./config/Foo.ini";   // file constants
//!     public static int SOME_VALUE;                                       // config fields
//!     public static void load()                                           // the body we read
//!     {
//!         final ConfigReader config = new ConfigReader(FOO_CONFIG_FILE);
//!         SOME_VALUE = config.getInt("SomeValue", 5);                     // "simple" statements
//!         ...                                                             // everything else
//!     }
//! }
//! ```
//!
//! What it offers: comment stripping that keeps line numbers, a split of the class into members,
//! a split of `load()` into top-level statements, the getter-call parser and the evaluator for
//! trivial default expressions.

use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------------------------
// Lexical helpers
// ---------------------------------------------------------------------------------------------

/// Index just after the string or char literal that starts at `start` (an unterminated literal
/// stops at the end of the line, so one bad quote cannot swallow the file).
pub fn skip_literal(b: &[u8], start: usize) -> usize {
    let quote = b[start];
    let mut j = start + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == quote => return j + 1,
            b'\n' => return j,
            _ => j += 1,
        }
    }
    b.len()
}

/// Removes `//` and `/* */` comments and `\r`, keeping every `\n` (so line numbers stay valid).
pub fn strip_comments(src: &str) -> Result<String> {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    if b[i] == b'\n' {
                        out.push(b'\n');
                    }
                    i += 1;
                }
                i += 2;
                out.push(b' ');
            }
            b'"' | b'\'' => {
                let end = skip_literal(b, i);
                out.extend_from_slice(&b[i..end.min(b.len())]);
                i = end;
            }
            b'\r' => i += 1,
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|e| anyhow!("source is not valid UTF-8 after stripping comments: {e}"))
}

/// 1-based line of byte offset `offset`.
pub fn line_at(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|&&c| c == b'\n')
        .count()
        + 1
}

/// Index of the bracket matching the one at `open` (`{`/`}` or `(`/`)`), skipping literals.
fn matching_close(text: &str, open: usize) -> Result<usize> {
    let b = text.as_bytes();
    let (o, c) = match b[open] {
        b'{' => (b'{', b'}'),
        b'(' => (b'(', b')'),
        other => bail!("internal: `{}` is not an opening bracket", other as char),
    };
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            x if x == o => depth += 1,
            x if x == c => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    bail!(
        "unbalanced `{}` opened at line {}",
        o as char,
        line_at(text, open)
    )
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// True when `s` is `[A-Z][A-Z0-9_]*`: the shape of every config constant name.
pub fn is_constant_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// All identifiers in `text` (outside string and char literals) with the byte before them, so a
/// caller can tell `NAME` from `OtherClass.NAME`.
pub fn identifiers(text: &str) -> Vec<(&str, Option<u8>)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = skip_literal(b, i),
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && is_ident_byte(b[i]) {
                    i += 1;
                }
                // Skip back over blanks to find what precedes the identifier.
                let mut p = start;
                let mut prev = None;
                while p > 0 {
                    p -= 1;
                    if !b[p].is_ascii_whitespace() {
                        prev = Some(b[p]);
                        break;
                    }
                }
                out.push((&text[start..i], prev));
            }
            c if c.is_ascii_digit() => {
                while i < b.len() && is_ident_byte(b[i]) {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Collapses runs of whitespace to one space.
pub fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------------------------
// Class structure
// ---------------------------------------------------------------------------------------------

/// A `public static TYPE NAME [= init];` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaField {
    pub name: String,
    pub java_type: String,
    pub init: Option<String>,
    pub line: usize,
}

/// A top-level statement of a method body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub text: String,
    pub line: usize,
}

/// What [`scan_class`] finds in one config class.
#[derive(Debug, Clone)]
pub struct ClassScan {
    pub name: String,
    /// `private static final String X_FILE = "...";` constants.
    pub constants: BTreeMap<String, String>,
    pub fields: Vec<JavaField>,
    /// Top-level statements of `public static void load()`.
    pub load: Vec<Statement>,
    /// Byte range of the `load()` body in the stripped text.
    pub load_range: (usize, usize),
    /// The comment-free source, line structure intact.
    pub text: String,
}

struct Member {
    header: String,
    body: Option<(usize, usize)>,
    /// Byte offset where the member starts.
    start: usize,
}

/// Splits the class body (`text[from..to]`) into members. Anything inside a member's braces or
/// parentheses belongs to it.
fn split_members(text: &str, from: usize, to: usize) -> Result<Vec<Member>> {
    let b = text.as_bytes();
    let mut members = Vec::new();
    let mut start = from;
    let mut i = from;
    let mut paren = 0i32;
    while i < to {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'{' if paren == 0 => {
                let close = matching_close(text, i)?;
                if text[start..i].contains('=') {
                    // An initializer such as `new int[] { 1, 2 }`: still the same member.
                    i = close + 1;
                    continue;
                }
                members.push(Member {
                    header: text[start..i].trim().to_string(),
                    body: Some((i + 1, close)),
                    start,
                });
                i = close + 1;
                start = i;
                continue;
            }
            b';' if paren == 0 => {
                members.push(Member {
                    header: text[start..i].trim().to_string(),
                    body: None,
                    start,
                });
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if !text[start..to].trim().is_empty() {
        bail!(
            "unterminated member at line {}: `{}`",
            line_at(text, start),
            squash(&text[start..to])
        );
    }
    Ok(members)
}

/// Splits a `{ ... }` body (`text[from..to]`) into top-level statements.
///
/// A statement ends at a `;` outside any brackets, or at the `}` that closes a block statement
/// (unless `else`, `catch` or `finally` follows, which continue it).
pub fn split_statements(text: &str, from: usize, to: usize) -> Result<Vec<Statement>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut start = from;
    let mut i = from;
    let (mut brace, mut paren, mut bracket) = (0i32, 0i32, 0i32);
    let push = |s: usize, e: usize, out: &mut Vec<Statement>| {
        let raw = &text[s..e];
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == ";" {
            return;
        }
        let lead = raw.len() - raw.trim_start().len();
        out.push(Statement {
            text: trimmed.to_string(),
            line: line_at(text, s + lead),
        });
    };
    while i < to {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            b'{' => brace += 1,
            b'}' => {
                brace -= 1;
                if brace < 0 {
                    bail!("unbalanced `}}` at line {}", line_at(text, i));
                }
                if brace == 0 && paren == 0 && bracket == 0 {
                    let rest = text[i + 1..to].trim_start();
                    let continues = ["else", "catch", "finally"].iter().any(|kw| {
                        rest.strip_prefix(kw)
                            .is_some_and(|r| !r.as_bytes().first().is_some_and(|&c| is_ident_byte(c)))
                    });
                    // `}` of an array initializer is followed by `;` or `)`: not a block end.
                    let terminated = rest.starts_with([';', ')', ',', '.']);
                    if !continues && !terminated {
                        push(start, i + 1, &mut out);
                        start = i + 1;
                    }
                }
            }
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => bracket += 1,
            b']' => bracket -= 1,
            b';' if brace == 0 && paren == 0 && bracket == 0 => {
                push(start, i + 1, &mut out);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if brace != 0 || paren != 0 || bracket != 0 {
        bail!("unbalanced brackets in the body at line {}", line_at(text, from));
    }
    if !text[start..to].trim().is_empty() {
        bail!(
            "statement without a terminator at line {}: `{}`",
            line_at(text, start),
            squash(&text[start..to])
        );
    }
    Ok(out)
}

const MODIFIERS: &[&str] = &[
    "public",
    "private",
    "protected",
    "static",
    "final",
    "volatile",
    "transient",
];

/// Splits `modifiers TYPE NAME` (a declaration without initializer) into its parts.
fn split_declaration(decl: &str) -> Result<(Vec<String>, String, String)> {
    let mut depth = 0i32;
    for c in decl.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => bail!("several declarators in one member: `{}`", squash(decl)),
            _ => {}
        }
    }
    let squashed = squash(decl);
    let (rest, name) = squashed
        .rsplit_once(' ')
        .ok_or_else(|| anyhow!("cannot read the declaration `{squashed}`"))?;
    let mut modifiers = Vec::new();
    let mut rest = rest;
    loop {
        let word_end = rest.find(' ').unwrap_or(rest.len());
        let word = &rest[..word_end];
        if MODIFIERS.contains(&word) {
            modifiers.push(word.to_string());
            rest = rest[word_end..].trim_start();
        } else {
            break;
        }
    }
    if rest.is_empty() {
        bail!("declaration without a type: `{squashed}`");
    }
    Ok((modifiers, rest.to_string(), name.to_string()))
}

/// Index of the first `=` that is an assignment (not `==`, `<=`, `>=`, `!=`), outside brackets.
pub fn find_assignment(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    let (mut paren, mut angle) = (0i32, 0i32);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            b'(' | b'[' => paren += 1,
            b')' | b']' => paren -= 1,
            b'<' => angle += 1,
            b'>' => angle -= 1,
            b'=' if paren == 0 && angle <= 0 => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if next != b'=' && !matches!(prev, b'=' | b'<' | b'>' | b'!') {
                    return Some(i);
                }
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Scans one config class.
pub fn scan_class(source: &str) -> Result<ClassScan> {
    let text = strip_comments(source)?;
    let class_kw = text
        .find("public class ")
        .ok_or_else(|| anyhow!("no `public class` found"))?;
    let after = &text[class_kw + "public class ".len()..];
    let name_end = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    let name = after[..name_end].to_string();
    let open = text[class_kw..]
        .find('{')
        .map(|p| p + class_kw)
        .ok_or_else(|| anyhow!("class {name} has no body"))?;
    let close = matching_close(&text, open)?;

    let mut constants = BTreeMap::new();
    let mut fields: Vec<JavaField> = Vec::new();
    let mut load_range = None;
    for member in split_members(&text, open + 1, close)? {
        let line = line_at(
            &text,
            member.start + (text[member.start..].len() - text[member.start..].trim_start().len()),
        );
        let header = squash(&member.header);
        let ctx = || format!("{name} line {line}: `{header}`");
        if header.is_empty() {
            continue;
        }
        let is_nested_type = ["class ", "interface ", "enum ", "record "]
            .iter()
            .any(|kw| header.starts_with(kw) || header.contains(&format!(" {kw}")));
        if is_nested_type {
            continue;
        }
        if let Some((from, to)) = member.body {
            if !header.contains('(') {
                bail!("{}: unclassified block member (static initializer?)", ctx());
            }
            if header == "public static void load()" && load_range.replace((from, to)).is_some() {
                bail!("{}: two load() methods", ctx());
            }
            continue;
        }
        // A field.
        let (decl, init) = match find_assignment(&member.header) {
            Some(eq) => (
                &member.header[..eq],
                Some(member.header[eq + 1..].trim().to_string()),
            ),
            None => (member.header.as_str(), None),
        };
        let (modifiers, java_type, field_name) = split_declaration(decl).with_context(ctx)?;
        let has = |m: &str| modifiers.iter().any(|x| x == m);
        if has("public") && has("static") && !has("final") {
            if !is_constant_name(&field_name) {
                bail!(
                    "{}: config field `{field_name}` is not an UPPER_SNAKE name",
                    ctx()
                );
            }
            if fields.iter().any(|f| f.name == field_name) {
                bail!("{}: field `{field_name}` is declared twice", ctx());
            }
            fields.push(JavaField {
                name: field_name,
                java_type,
                init,
                line,
            });
        } else if has("private") && has("static") && has("final") && java_type == "String" {
            let value = init
                .as_deref()
                .and_then(|i| parse_string_literal(i).filter(|(_, rest)| rest.trim().is_empty()))
                .map(|(s, _)| s)
                .ok_or_else(|| anyhow!("{}: file constant without a plain string literal", ctx()))?;
            constants.insert(field_name, value);
        } else if has("private") && has("static") && has("final") && java_type == "Logger" {
            // `LOGGER`
        } else {
            bail!("{}: member is neither a config field nor a known constant", ctx());
        }
    }
    let load_range = load_range.ok_or_else(|| anyhow!("{name}: no `public static void load()`"))?;
    let load =
        split_statements(&text, load_range.0, load_range.1).with_context(|| format!("{name}.load()"))?;
    Ok(ClassScan {
        name,
        constants,
        fields,
        load,
        load_range,
        text,
    })
}

// ---------------------------------------------------------------------------------------------
// Getter calls and literals
// ---------------------------------------------------------------------------------------------

/// `reader.getKind("Key", default)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetterCall {
    pub reader: String,
    /// The part after `get`: `Int`, `Boolean`, `String`, `IntArray`, ...
    pub kind: String,
    /// The first argument, unescaped when it is a string literal, otherwise the raw expression.
    pub key: String,
    pub key_is_literal: bool,
    /// The second argument as written, `None` when the call has only one argument.
    pub default_expr: Option<String>,
    /// Byte offset just after the closing `)`.
    pub end: usize,
}

/// Parses `"text"` at the start of `s`, returning the unescaped text and the rest.
pub fn parse_string_literal(s: &str) -> Option<(String, &str)> {
    let s_trim = s.trim_start();
    let b = s_trim.as_bytes();
    if b.first() != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = s_trim[1..].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s_trim[1 + i + 1..])),
            '\\' => {
                let (_, e) = chars.next()?;
                match e {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '"' => out.push('"'),
                    '\'' => out.push('\''),
                    '\\' => out.push('\\'),
                    'u' => {
                        let hex: String = (&mut chars).take(4).map(|(_, c)| c).collect();
                        let code = u32::from_str_radix(&hex, 16).ok()?;
                        out.push(char::from_u32(code)?);
                    }
                    // Octal escapes and the like: not worth guessing.
                    _ => return None,
                }
            }
            _ => out.push(c),
        }
    }
    None
}

/// Parses the getter call that starts exactly at byte `at` of `text` (the reader name).
pub fn parse_getter(text: &str, at: usize) -> Option<GetterCall> {
    let b = text.as_bytes();
    let mut i = at;
    while i < b.len() && is_ident_byte(b[i]) {
        i += 1;
    }
    let reader = &text[at..i];
    if reader.is_empty() || !text[i..].starts_with(".get") {
        return None;
    }
    i += ".get".len();
    let kind_start = i;
    while i < b.len() && is_ident_byte(b[i]) {
        i += 1;
    }
    let kind = &text[kind_start..i];
    if !kind.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if b.get(i) != Some(&b'(') {
        return None;
    }
    let close = matching_close(text, i).ok()?;
    let args = split_args(&text[i + 1..close]);
    let first = args.first()?.trim();
    if first.is_empty() {
        return None; // `getPath()`: not a ConfigReader getter
    }
    let (key, key_is_literal) = match parse_string_literal(first) {
        Some((k, rest)) if rest.trim().is_empty() => (k, true),
        _ => (first.to_string(), false),
    };
    Some(GetterCall {
        reader: reader.to_string(),
        kind: kind.to_string(),
        key,
        key_is_literal,
        default_expr: args.get(1).map(|a| a.trim().to_string()),
        end: close + 1,
    })
}

/// Splits call arguments at top-level commas.
fn split_args(args: &str) -> Vec<String> {
    let b = args.as_bytes();
    let mut out = Vec::new();
    let (mut depth, mut start, mut i) = (0i32, 0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                out.push(args[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(args[start..].to_string());
    out
}

/// Every call `reader.getX(...)` in `text` whose reader is one of `readers`.
pub fn find_getters(text: &str, readers: &[String]) -> Vec<GetterCall> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                i = skip_literal(b, i);
                continue;
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && is_ident_byte(b[i]) {
                    i += 1;
                }
                let preceded_by_dot = text[..start].trim_end().ends_with('.');
                if !preceded_by_dot && readers.iter().any(|r| r == &text[start..i]) {
                    if let Some(call) = parse_getter(text, start) {
                        out.push(call);
                    }
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// A default value we can translate.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Bool(bool),
    Int(i128),
    /// Rust source text of the float (always contains `.` or an exponent).
    Float(String),
    Str(String),
}

/// The getters `IniFile` has a typed twin for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Boolean,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    String,
}

impl Kind {
    pub fn from_getter(kind: &str) -> Option<Self> {
        Some(match kind {
            "Boolean" => Self::Boolean,
            "Byte" => Self::Byte,
            "Short" => Self::Short,
            "Int" => Self::Int,
            "Long" => Self::Long,
            "Float" => Self::Float,
            "Double" => Self::Double,
            "String" => Self::String,
            _ => return None,
        })
    }

    /// The Java primitive (or `String`) a field of this kind must be declared as.
    pub fn java_type(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Byte => "byte",
            Self::Short => "short",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
            Self::String => "String",
        }
    }

    pub fn rust_type(self) -> &'static str {
        match self {
            Self::Boolean => "bool",
            Self::Byte => "i8",
            Self::Short => "i16",
            Self::Int => "i32",
            Self::Long => "i64",
            Self::Float => "f32",
            Self::Double => "f64",
            Self::String => "String",
        }
    }

    /// Name of the `IniFile` method.
    pub fn rust_getter(self) -> &'static str {
        match self {
            Self::Boolean => "get_bool",
            Self::Byte => "get_byte",
            Self::Short => "get_short",
            Self::Int => "get_int",
            Self::Long => "get_long",
            Self::Float => "get_float",
            Self::Double => "get_double",
            Self::String => "get_string",
        }
    }

    /// The kind a Java field type names (`boolean`, `int`, `String`, ...).
    pub fn from_java_type(java_type: &str) -> Option<Self> {
        [
            Self::Boolean,
            Self::Byte,
            Self::Short,
            Self::Int,
            Self::Long,
            Self::Float,
            Self::Double,
            Self::String,
        ]
        .into_iter()
        .find(|k| k.java_type() == java_type)
    }

    /// True when Rust has `Target::from(Self)`, i.e. Java's implicit widening is lossless.
    pub fn converts_to(self, target: Self) -> bool {
        use Kind::{Byte, Double, Float, Int, Long, Short};
        self == target
            || matches!(
                (self, target),
                (Byte, Short | Int | Long | Float | Double)
                    | (Short, Int | Long | Float | Double)
                    | (Int, Long | Double)
                    | (Float, Double)
            )
    }

    fn int_range(self) -> Option<(i128, i128)> {
        match self {
            Self::Byte => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
            Self::Short => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
            Self::Int => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
            Self::Long => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
            _ => None,
        }
    }
}

fn parse_int_literal(s: &str) -> Option<i128> {
    let s = s.replace('_', "");
    let s = s.strip_suffix(['L', 'l']).unwrap_or(&s);
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return i128::from_str_radix(hex, 16).ok();
    }
    if s.len() > 1 && s.starts_with('0') {
        return None; // Java octal: not worth guessing
    }
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn parse_float_literal(s: &str) -> Option<String> {
    let s = s.replace('_', "");
    let body = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(&s);
    if body.is_empty() || body.starts_with(['e', 'E']) {
        return None;
    }
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(p) => (&body[..p], Some(&body[p..])),
        None => (body, None),
    };
    let valid_exp = exponent.is_none_or(|e| {
        let digits = e[1..].trim_start_matches(['+', '-']);
        !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
    });
    let mut parts = mantissa.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next();
    let digits_ok = |p: &str| p.bytes().all(|c| c.is_ascii_digit());
    if !valid_exp
        || !digits_ok(int_part)
        || !frac_part.is_none_or(digits_ok)
        || (int_part.is_empty() && frac_part.is_none_or(str::is_empty))
    {
        return None;
    }
    // Rust needs digits on both sides of the dot.
    let int_text = if int_part.is_empty() { "0" } else { int_part };
    let frac_text = match frac_part {
        Some(f) if !f.is_empty() => f,
        _ => "0",
    };
    Some(format!("{int_text}.{frac_text}{}", exponent.unwrap_or("")))
}

/// Evaluates the default expression of a getter when it is trivial: a literal (optionally negated
/// and cast to the field's own type) or `Integer.MAX_VALUE` and friends. Returns `None` for
/// anything else, which sends the field to the manual list.
pub fn eval_default(kind: Kind, expr: &str) -> Option<Literal> {
    let mut e = expr.trim();
    if kind == Kind::String {
        let (s, rest) = parse_string_literal(e)?;
        return rest.trim().is_empty().then_some(Literal::Str(s));
    }
    if kind == Kind::Boolean {
        return match e {
            "true" => Some(Literal::Bool(true)),
            "false" => Some(Literal::Bool(false)),
            _ => None,
        };
    }
    // `(byte) 20`
    if let Some(rest) = e.strip_prefix('(') {
        let (cast, rest) = rest.split_once(')')?;
        let cast = cast.trim();
        let allowed = match kind {
            Kind::Byte => ["byte"].as_slice(),
            Kind::Short => ["short"].as_slice(),
            Kind::Int => ["int"].as_slice(),
            Kind::Long => ["long"].as_slice(),
            Kind::Float => ["float"].as_slice(),
            Kind::Double => ["double"].as_slice(),
            _ => [].as_slice(),
        };
        if !allowed.contains(&cast) {
            return None;
        }
        e = rest.trim();
    }
    let (negative, body) = match e.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, e),
    };
    if let Some((lo, hi)) = kind.int_range() {
        let magnitude = match body {
            "Integer.MAX_VALUE" => i128::from(i32::MAX),
            "Integer.MIN_VALUE" if !negative => i128::from(i32::MIN),
            "Long.MAX_VALUE" => i128::from(i64::MAX),
            "Long.MIN_VALUE" if !negative => i128::from(i64::MIN),
            "Short.MAX_VALUE" => i128::from(i16::MAX),
            "Short.MIN_VALUE" if !negative => i128::from(i16::MIN),
            "Byte.MAX_VALUE" => i128::from(i8::MAX),
            "Byte.MIN_VALUE" if !negative => i128::from(i8::MIN),
            other => parse_int_literal(other)?,
        };
        let value = if negative { -magnitude } else { magnitude };
        // Java rejects an out-of-range literal at compile time; so a range error here would mean
        // we misread something.
        return (lo..=hi).contains(&value).then_some(Literal::Int(value));
    }
    // Float / Double: an integer literal is fine too (`getDouble("X", 100)`).
    let text = if let Some(int) = parse_int_literal(body) {
        // Integer literals above 2^53 would lose precision silently: refuse.
        if int.abs() > (1 << 53) {
            return None;
        }
        format!("{int}.0")
    } else {
        parse_float_literal(body)?
    };
    let finite = match kind {
        Kind::Float => text.parse::<f32>().is_ok_and(f32::is_finite),
        _ => text.parse::<f64>().is_ok_and(f64::is_finite),
    };
    finite.then(|| Literal::Float(if negative { format!("-{text}") } else { text }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comments_and_keeps_lines() {
        let src = "a // x\n/* b\n c */ d \"//not\" 'e'\r\n";
        assert_eq!(strip_comments(src).unwrap(), "a \n\n  d \"//not\" 'e'\n");
    }

    #[test]
    fn splits_statements_with_blocks() {
        let text = "{ A = 1; for (String s : x)\n{ y.add(s); }\n try { z(); } catch (E e) { w(); } B = new int[] { 1, 2 }; }";
        let end = text.len() - 1;
        let stmts = split_statements(text, 1, end).unwrap();
        let texts: Vec<_> = stmts.iter().map(|s| squash(&s.text)).collect();
        assert_eq!(
            texts,
            [
                "A = 1;",
                "for (String s : x) { y.add(s); }",
                "try { z(); } catch (E e) { w(); }",
                "B = new int[] { 1, 2 };",
            ]
        );
    }

    #[test]
    fn unterminated_statement_is_an_error() {
        let text = "{ A = 1 }";
        assert!(split_statements(text, 1, text.len() - 1).is_err());
    }

    const SAMPLE: &str = r#"
package x;
/** doc */
public class FooConfig
{
	private static final Logger LOGGER = Logger.getLogger(FooConfig.class.getName());
	private static final String FOO_CONFIG_FILE = "./config/Foo.ini";
	public static int A;
	public static Map<Integer, Integer> B;
	public static Set<Integer> C = new HashSet<>();
	public static void load()
	{
		final ConfigReader config = new ConfigReader(FOO_CONFIG_FILE);
		A = config.getInt("A", 5);
	}
	private static int helper(int x) { return x; }
	private static class Inner { int q; }
}
"#;

    #[test]
    fn scans_a_class() {
        let class = scan_class(SAMPLE).unwrap();
        assert_eq!(class.name, "FooConfig");
        assert_eq!(class.constants["FOO_CONFIG_FILE"], "./config/Foo.ini");
        let fields: Vec<_> = class
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f.java_type.as_str()))
            .collect();
        assert_eq!(
            fields,
            [
                ("A", "int"),
                ("B", "Map<Integer, Integer>"),
                ("C", "Set<Integer>")
            ]
        );
        assert_eq!(class.fields[2].init.as_deref(), Some("new HashSet<>()"));
        assert_eq!(class.load.len(), 2);
        assert_eq!(class.load[1].text, "A = config.getInt(\"A\", 5);");
    }

    #[test]
    fn unknown_member_is_an_error() {
        let src = SAMPLE.replace("public static int A;", "protected int a;");
        assert!(
            scan_class(&src)
                .unwrap_err()
                .to_string()
                .contains("neither a config field")
        );
    }

    #[test]
    fn parses_getter_calls() {
        let readers = vec!["config".to_string()];
        let text = r#"X = config.getString("K\"q", "a, b").split(",");"#;
        let calls = find_getters(text, &readers);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].kind, "String");
        assert_eq!(calls[0].key, "K\"q");
        assert_eq!(calls[0].default_expr.as_deref(), Some("\"a, b\""));
        assert_eq!(&text[calls[0].end..], ".split(\",\");");
        // Other receivers and no-argument getters are not reader calls.
        assert!(find_getters("a.getPath(); other.getInt(\"K\", 1);", &readers).is_empty());
        assert!(find_getters("foo.config.getInt(\"K\", 1);", &readers).is_empty());
    }

    #[test]
    fn evaluates_trivial_defaults() {
        use Literal::{Bool, Float, Int, Str};
        assert_eq!(eval_default(Kind::Int, "600"), Some(Int(600)));
        assert_eq!(eval_default(Kind::Int, " -1 "), Some(Int(-1)));
        assert_eq!(
            eval_default(Kind::Int, "Integer.MAX_VALUE"),
            Some(Int(2_147_483_647))
        );
        assert_eq!(
            eval_default(Kind::Int, "-Integer.MAX_VALUE"),
            Some(Int(-2_147_483_647))
        );
        assert_eq!(
            eval_default(Kind::Int, "Integer.MIN_VALUE"),
            Some(Int(-2_147_483_648))
        );
        assert_eq!(eval_default(Kind::Int, "0x10"), Some(Int(16)));
        assert_eq!(eval_default(Kind::Int, "010"), None);
        assert_eq!(eval_default(Kind::Int, "3000000000"), None);
        assert_eq!(eval_default(Kind::Int, "SOME_CONSTANT"), None);
        assert_eq!(eval_default(Kind::Int, "60 * 1000"), None);
        assert_eq!(
            eval_default(Kind::Long, "50000000000L"),
            Some(Int(50_000_000_000))
        );
        assert_eq!(
            eval_default(Kind::Long, "Long.MAX_VALUE"),
            Some(Int(i128::from(i64::MAX)))
        );
        assert_eq!(eval_default(Kind::Byte, "(byte) 20"), Some(Int(20)));
        assert_eq!(eval_default(Kind::Byte, "(short) 20"), None);
        assert_eq!(eval_default(Kind::Byte, "200"), None);
        assert_eq!(eval_default(Kind::Boolean, "true"), Some(Bool(true)));
        assert_eq!(eval_default(Kind::Boolean, "TRUE"), None);
        assert_eq!(eval_default(Kind::Float, "0.6f"), Some(Float("0.6".into())));
        assert_eq!(eval_default(Kind::Float, "1"), Some(Float("1.0".into())));
        assert_eq!(eval_default(Kind::Double, "100"), Some(Float("100.0".into())));
        assert_eq!(eval_default(Kind::Double, "-.5"), Some(Float("-0.5".into())));
        assert_eq!(eval_default(Kind::Double, "5.d"), Some(Float("5.0".into())));
        assert_eq!(eval_default(Kind::Double, "1e3"), Some(Float("1.0e3".into())));
        assert_eq!(eval_default(Kind::Double, "x"), None);
        assert_eq!(eval_default(Kind::String, r#""a\tb""#), Some(Str("a\tb".into())));
        assert_eq!(eval_default(Kind::String, r#""a" + "b""#), None);
        assert_eq!(eval_default(Kind::String, "SOME_CONSTANT"), None);
    }
}
