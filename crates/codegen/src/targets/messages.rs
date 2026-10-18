//! Target `messages`: `SystemMessageId` and `NpcStringId` tables.
//!
//! Java: gameserver/network/{SystemMessageId,NpcStringId,ClientString}.java
//! Output: crates/l2-protocol-game/src/generated/{system_message_id,npc_string_id}.rs (+ generated/mod.rs)
//!
//! Both Java classes are one long list of
//!
//! ```text
//! @ClientString(id = 1, message = "The server will be coming down in $s1 second(s).")
//! public static SystemMessageId THE_SERVER_WILL_BE_COMING_DOWN_IN_S1_SECOND_S;
//! ```
//!
//! The parser accepts exactly that shape (an annotation line followed by its field line) and fails
//! on anything else, so a change in the Java sources is never silently misread.

use super::opcodes::camel_case;
use super::{Output, game_generated_mod, render};
use anyhow::{Context, Result, anyhow, bail};
use quote::{ToTokens, format_ident, quote};
use std::collections::HashMap;
use std::path::Path;

const NETWORK_DIR: &str = "java/org/l2jmobius/gameserver/network";

/// The Java source text of the two files this target reads.
pub struct Sources {
    pub system_message_id: String,
    pub npc_string_id: String,
}

pub fn read_sources(l2j_root: &Path) -> Result<Sources> {
    let read = |file: &str| -> Result<String> {
        let path = l2j_root.join(NETWORK_DIR).join(file);
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    };
    Ok(Sources {
        system_message_id: read("SystemMessageId.java")?,
        npc_string_id: read("NpcStringId.java")?,
    })
}

/// One `@ClientString` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    /// The Java field name, e.g. `S1_DOES_NOT_EXIST`.
    pub name: String,
    pub message: String,
    /// Number of `$s`/`$c` parameters. Derived from the field *name*, see [`param_count`].
    pub params: u8,
}

/// Port of `parseMessageParameters(String name)` (identical in both Java classes).
///
/// NOTE(java-quirk): Java looks at the constant's *name*, not at the message text: the highest
/// digit that follows a `C` or `S` (`..._S1_..._S2` -> 2). A word that ends in `S` or `C` right
/// before a digit would count too. We port that as is, because the SystemMessage packet code
/// relies on this number.
pub fn param_count(name: &str) -> u8 {
    let chars: Vec<char> = name.chars().collect();
    let mut count = 0u8;
    let mut i = 0;
    while i + 1 < chars.len() {
        if (chars[i] == 'C' || chars[i] == 'S')
            && let Some(digit) = chars[i + 1].to_digit(10)
        {
            count = count.max(digit as u8);
            i += 1; // Java skips the digit
        }
        i += 1;
    }
    count
}

/// Decodes the Java string literal body (after the opening quote). Returns the text and the rest of
/// the line after the closing quote.
fn parse_java_string(input: &str) -> Result<(String, &str)> {
    let mut out = String::new();
    let mut chars = input.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &input[i + 1..])),
            '\\' => {
                let (_, e) = chars
                    .next()
                    .ok_or_else(|| anyhow!("string ends after a backslash"))?;
                match e {
                    '\\' | '"' | '\'' => out.push(e),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    other => bail!("unsupported escape `\\{other}` in a string literal"),
                }
            }
            c => out.push(c),
        }
    }
    bail!("unterminated string literal")
}

/// Parses `@ClientString(id = N, message = "...")`.
fn parse_annotation(line: &str) -> Result<(i32, String)> {
    let rest = line
        .strip_prefix("@ClientString(id = ")
        .ok_or_else(|| anyhow!("not an @ClientString line"))?;
    let (id, rest) = rest
        .split_once(", message = \"")
        .ok_or_else(|| anyhow!("expected `, message = \"`"))?;
    let id: i32 = id.trim().parse().with_context(|| format!("bad id `{id}`"))?;
    let (message, rest) = parse_java_string(rest)?;
    if rest != ")" {
        bail!("unexpected text after the message: `{rest}`");
    }
    Ok((id, message))
}

/// Parses every `@ClientString` entry of one Java class (`SystemMessageId` or `NpcStringId`).
pub fn parse_entries(source: &str, class: &str) -> Result<Vec<Entry>> {
    let field_prefix = format!("public static {class} ");
    let mut entries = Vec::new();
    let mut lines = source.lines().enumerate();
    let mut annotations = 0usize;
    let mut loose_fields = 0usize;
    while let Some((n, raw)) = lines.next() {
        let line = raw.trim();
        let at = format!("{class}.java line {}", n + 1);
        if line.starts_with("@ClientString") {
            annotations += 1;
            let (id, message) = parse_annotation(line).with_context(|| at.clone())?;
            let (_, field) = lines
                .by_ref()
                .find(|(_, l)| !l.trim().is_empty())
                .ok_or_else(|| anyhow!("{at}: annotation without a field"))?;
            let name = field
                .trim()
                .strip_prefix(&field_prefix)
                .and_then(|r| r.strip_suffix(';'))
                .ok_or_else(|| {
                    anyhow!("{at}: expected `{field_prefix}NAME;` after the annotation, got `{field}`")
                })?;
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                bail!("{at}: unexpected field name `{name}`");
            }
            entries.push(Entry {
                id,
                name: name.to_string(),
                message,
                params: param_count(name),
            });
        } else if line.starts_with(&field_prefix) && line.ends_with(';') && !line.contains('(') {
            // (methods like `getSystemMessageId(int id)` share the prefix and are not fields)
            // A field without an annotation stays `null` in Java; there is none today and we
            // would rather know when one appears.
            loose_fields += 1;
        }
    }
    if loose_fields != 0 {
        bail!("{class}: {loose_fields} field(s) without @ClientString");
    }
    if entries.len() != annotations {
        bail!("{class}: parsed {} of {annotations} entries", entries.len());
    }
    if entries.is_empty() {
        bail!("{class}: no @ClientString entries found");
    }
    Ok(entries)
}

struct Row<'a> {
    entry: &'a Entry,
    variant: String,
}

/// Rust variant names (CamelCase of the Java name), checked for uniqueness. Ids must be unique too.
fn rows<'a>(class: &str, entries: &'a [Entry]) -> Result<Vec<Row<'a>>> {
    let mut names: HashMap<String, &str> = HashMap::new();
    let mut ids: HashMap<i32, &str> = HashMap::new();
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let variant = camel_case(&entry.name);
        // `Self` is the only capitalised Rust keyword.
        if variant.is_empty() || variant == "Self" || variant.starts_with(|c: char| c.is_ascii_digit()) {
            bail!(
                "{class}.{}: cannot make a Rust variant name out of it",
                entry.name
            );
        }
        if let Some(other) = names.insert(variant.clone(), &entry.name) {
            bail!("{class}: {} and {other} both become `{variant}`", entry.name);
        }
        if let Some(other) = ids.insert(entry.id, &entry.name) {
            bail!(
                "{class}.{}: id {} is already used by {other}",
                entry.name,
                entry.id
            );
        }
        out.push(Row { entry, variant });
    }
    Ok(out)
}

fn int_lit(value: i32) -> Result<syn::LitInt> {
    Ok(syn::parse_str(&value.to_string())?)
}

fn class_tokens(class: &str, doc: &str, rows: &[Row]) -> Result<impl ToTokens + use<>> {
    let name = format_ident!("{class}");
    let doc = format!(" {doc}");
    let variants: Vec<_> = rows.iter().map(|r| format_ident!("{}", r.variant)).collect();
    let ids = rows
        .iter()
        .map(|r| int_lit(r.entry.id))
        .collect::<Result<Vec<_>>>()?;
    let java: Vec<_> = rows.iter().map(|r| r.entry.name.as_str()).collect();
    let messages: Vec<_> = rows.iter().map(|r| r.entry.message.as_str()).collect();
    let params = rows
        .iter()
        .map(|r| int_lit(i32::from(r.entry.params)))
        .collect::<Result<Vec<_>>>()?;
    let count = rows.len();

    Ok(quote! {
        #[doc = #doc]
        #[repr(i32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum #name {
            #(#variants = #ids,)*
        }

        impl #name {
            /// Every entry, in Java declaration order.
            pub const ALL: [Self; #count] = [#(Self::#variants,)*];

            /// The numeric id the client knows this text by.
            pub const fn id(self) -> i32 {
                self as i32
            }

            /// The Java constant name, e.g. `S1_DOES_NOT_EXIST`.
            pub const fn name(self) -> &'static str {
                self.row().0
            }

            /// The default (English) client text, with `$s1`/`$c1` style placeholders.
            pub const fn message(self) -> &'static str {
                self.row().1
            }

            /// How many parameters the Java server sends with this id (`getParamCount()`).
            pub const fn param_count(self) -> u8 {
                self.row().2
            }

            /// Looks an entry up by id.
            pub const fn from_id(id: i32) -> Option<Self> {
                match id {
                    #(#ids => Some(Self::#variants),)*
                    _ => None,
                }
            }

            const fn row(self) -> (&'static str, &'static str, u8) {
                match self {
                    #(Self::#variants => (#java, #messages, #params),)*
                }
            }
        }
    })
}

/// Pure generator: Java text in, files out.
pub fn generate(src: &Sources) -> Result<Vec<Output>> {
    let sm = parse_entries(&src.system_message_id, "SystemMessageId")?;
    let ns = parse_entries(&src.npc_string_id, "NpcStringId")?;
    let sm_tokens = class_tokens(
        "SystemMessageId",
        "Client system message ids (`SystemMessage` packet). Generated from the Java `@ClientString` table.",
        &rows("SystemMessageId", &sm)?,
    )?;
    let ns_tokens = class_tokens(
        "NpcStringId",
        "Client NPC string ids (`NpcSay`, `ExShowScreenMessage`, ...). Generated from the Java `@ClientString` table.",
        &rows("NpcStringId", &ns)?,
    )?;
    Ok(vec![
        game_generated_mod()?,
        Output {
            path: "crates/l2-protocol-game/src/generated/system_message_id.rs".into(),
            contents: render(
                "// Source: Java gameserver/network/SystemMessageId.java\n// Regenerate: cargo run -p codegen -- messages",
                sm_tokens,
            )?,
        },
        Output {
            path: "crates/l2-protocol-game/src/generated/npc_string_id.rs".into(),
            contents: render(
                "// Source: Java gameserver/network/NpcStringId.java\n// Regenerate: cargo run -p codegen -- messages",
                ns_tokens,
            )?,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Sources {
        Sources {
            system_message_id: "package x;\npublic class SystemMessageId\n{\n\
                \t@ClientString(id = 0, message = \"You have been disconnected.\")\n\
                \tpublic static SystemMessageId YOU_HAVE_BEEN_DISCONNECTED;\n\t\n\
                \t@ClientString(id = 1, message = \"$s1 gave \\\"$c2\\\" to $s3.\")\n\
                \tpublic static SystemMessageId S1_GAVE_C2_TO_S3;\n}\n"
                .to_string(),
            npc_string_id: "package x;\npublic class NpcStringId\n{\n\
                \t@ClientString(id = 10, message = \"a \\\\n b\")\n\
                \tpublic static NpcStringId HELLO;\n}\n"
                .to_string(),
        }
    }

    #[test]
    fn param_count_ports_java_parse_message_parameters() {
        assert_eq!(param_count("YOU_HAVE_BEEN_DISCONNECTED"), 0);
        assert_eq!(param_count("S1_GAVE_C2_TO_S3"), 3);
        assert_eq!(
            param_count("THE_SERVER_WILL_BE_COMING_DOWN_IN_S1_SECOND_S_PLEASE"),
            1
        );
        // The digit is consumed with its letter: `SS2` reads S, then S2.
        assert_eq!(param_count("SS2"), 2);
        // A trailing S/C has no next char.
        assert_eq!(param_count("ABS"), 0);
        // Only the letters C and S count.
        assert_eq!(param_count("A1_B2"), 0);
    }

    #[test]
    fn parses_escapes_and_entries() {
        let sm = parse_entries(&sample().system_message_id, "SystemMessageId").unwrap();
        assert_eq!(sm.len(), 2);
        assert_eq!(sm[1].message, "$s1 gave \"$c2\" to $s3.");
        assert_eq!(sm[1].params, 3);
        let ns = parse_entries(&sample().npc_string_id, "NpcStringId").unwrap();
        assert_eq!(ns[0].message, "a \\n b"); // Java source `\\n` is a backslash and an `n`
    }

    #[test]
    fn output_is_deterministic_and_has_pieces() {
        let a = generate(&sample()).unwrap();
        assert_eq!(a, generate(&sample()).unwrap());
        assert!(a.iter().all(|o| o.contents.starts_with(super::super::HEADER)));
        let text: String = a
            .iter()
            .find(|o| o.path.ends_with("system_message_id.rs"))
            .unwrap()
            .contents
            .split_whitespace()
            .collect();
        assert!(text.contains("S1GaveC2ToS3=1,"), "{text}");
        assert!(text.contains("1=>Some(Self::S1GaveC2ToS3)"));
        assert!(text.contains("Self::S1GaveC2ToS3=>(\"S1_GAVE_C2_TO_S3\""));
    }

    #[test]
    fn rejects_bad_input() {
        let bad = |s: &Sources| generate(s).is_err();

        let mut src = sample();
        src.system_message_id = src.system_message_id.replace("id = 1,", "id = 0,");
        assert!(bad(&src), "duplicate id");

        let mut src = sample();
        src.system_message_id = src
            .system_message_id
            .replace("S1_GAVE_C2_TO_S3", "YOU_HAVE_BEEN_DISCONNECTED_");
        assert!(bad(&src), "two names, one Rust variant");

        let mut src = sample();
        src.npc_string_id = src.npc_string_id.replace("HELLO", "SELF");
        assert!(bad(&src), "`Self` is a keyword");

        let mut src = sample();
        src.npc_string_id = src.npc_string_id.replace("\\\\n", "\\q");
        assert!(bad(&src), "unknown escape");

        let mut src = sample();
        src.npc_string_id = src
            .npc_string_id
            .replace("public static NpcStringId HELLO;", "int x;");
        assert!(bad(&src), "annotation without a field");

        let mut src = sample();
        src.npc_string_id.push_str("\tpublic static NpcStringId LOOSE;\n");
        assert!(bad(&src), "field without annotation");
    }
}
