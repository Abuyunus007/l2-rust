//! A small line parser for Java enum constant tables.
//!
//! This is deliberately not a Java parser. It understands exactly the shape the L2JMobius opcode
//! tables use and fails loudly on anything else, so a change in the Java sources can never be
//! silently misread:
//!
//! ```text
//! public enum ClientPackets
//! {
//!     LOGOUT(0x00, Logout::new, ConnectionState.AUTHENTICATED, ConnectionState.IN_GAME),
//!     SOME_ID(0x0E),            // arguments are optional (`NAME,` alone is fine too)
//!     LAST(0xD0, null, ConnectionState.values()); // `;` ends the constant list
//!     ...fields and methods are ignored from here on...
//! }
//! ```
//!
//! Rules:
//! - Comments (`//` and `/* */`) are removed first. String literals are skipped over, and are not
//!   allowed inside the constant list.
//! - Inside the constant list every non-blank line must be exactly one constant:
//!   `IDENT`, optionally followed by a parenthesised, comma separated argument list, then `,` or `;`.
//! - The `;` terminator ends the list. Reaching the end of the file first is an error.

use anyhow::{Context, Result, bail};

/// One enum constant: its name and the raw, trimmed text of each constructor argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    pub name: String,
    pub args: Vec<String>,
    /// 1-based source line, for error messages.
    pub line: usize,
}

/// Removes `//` and `/* */` comments, keeping the line structure (one output line per input line).
fn strip_comments(source: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = source.chars().peekable();
    let (mut in_block, mut in_line, mut in_string) = (false, false, false);
    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line = false;
            in_string = false; // Java strings can't span lines; recover instead of eating the file
            lines.push(std::mem::take(&mut current));
            continue;
        }
        if in_line {
            continue;
        }
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if in_string {
            current.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => in_line = true,
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block = true;
            }
            '"' => {
                in_string = true;
                current.push(c);
            }
            '\r' => {}
            _ => current.push(c),
        }
    }
    lines.push(current);
    lines
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Splits `a, b(c, d), e` on top-level commas.
fn split_args(args: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    bail!("unbalanced ')' in arguments `{args}`");
                }
            }
            ',' if depth == 0 => {
                out.push(args[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        bail!("unbalanced '(' in arguments `{args}`");
    }
    let last = args[start..].trim();
    if !last.is_empty() || !out.is_empty() {
        out.push(last.to_string());
    }
    if out.iter().any(String::is_empty) {
        bail!("empty argument in `{args}`");
    }
    Ok(out)
}

fn parse_constant(text: &str, line: usize) -> Result<(Constant, bool)> {
    let (body, last) = if let Some(body) = text.strip_suffix(',') {
        (body.trim_end(), false)
    } else if let Some(body) = text.strip_suffix(';') {
        (body.trim_end(), true)
    } else {
        bail!("expected a constant ending in ',' or ';', got `{text}`");
    };
    if body.contains('"') {
        bail!("string literals are not supported in constant lists: `{text}`");
    }
    let (name, args) = match body.find('(') {
        None => (body, Vec::new()),
        Some(open) => {
            let Some(inner) = body[open + 1..].strip_suffix(')') else {
                bail!("expected `)` at the end of `{text}`");
            };
            (body[..open].trim_end(), split_args(inner)?)
        }
    };
    if !is_ident(name) {
        bail!("`{name}` is not a valid constant name in `{text}`");
    }
    Ok((
        Constant {
            name: name.to_string(),
            args,
            line,
        },
        last,
    ))
}

/// Parses the constant list of `public enum <enum_name>` from a Java source file.
pub fn parse_enum(source: &str, enum_name: &str) -> Result<Vec<Constant>> {
    let lines = strip_comments(source);
    let mut iter = lines.iter().enumerate();

    // Find the enum declaration.
    let header = format!("enum {enum_name}");
    let mut opened = false;
    let mut found = false;
    for (_, line) in iter.by_ref() {
        let t = line.trim();
        if let Some(pos) = t.find(&header) {
            let rest = t[pos + header.len()..].trim();
            if rest.is_empty() || rest == "{" {
                opened = rest == "{";
                found = true;
                break;
            }
        }
    }
    if !found {
        bail!("`enum {enum_name}` not found");
    }
    // `{` may sit on its own line (Allman style, as in the Mobius sources).
    if !opened {
        for (_, line) in iter.by_ref() {
            match line.trim() {
                "" => {}
                "{" => break,
                other => bail!("expected `{{` after `enum {enum_name}`, got `{other}`"),
            }
        }
    }

    let mut constants = Vec::new();
    for (index, line) in iter {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let (constant, last) =
            parse_constant(text, index + 1).with_context(|| format!("{enum_name}: line {}", index + 1))?;
        constants.push(constant);
        if last {
            return Ok(constants);
        }
    }
    bail!("{enum_name}: constant list is not terminated by ';'")
}

/// Parses a Java integer literal (`0x1A`, `26`).
pub fn parse_int(text: &str) -> Result<u32> {
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse(),
    };
    value.with_context(|| format!("`{text}` is not an integer literal"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> Result<Vec<Constant>> {
        parse_enum(
            &format!("/* license */\npackage x;\npublic enum Sample\n{{\n{body}\n}}\n"),
            "Sample",
        )
    }

    #[test]
    fn single_state_line() {
        let c = parse("\tATTACK(0x01, AttackRequest::new, ConnectionState.IN_GAME);").unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "ATTACK");
        assert_eq!(
            c[0].args,
            ["0x01", "AttackRequest::new", "ConnectionState.IN_GAME"]
        );
    }

    #[test]
    fn multi_state_and_values_call() {
        let c = parse(
            "\tLOGOUT(0x00, Logout::new, ConnectionState.AUTHENTICATED, ConnectionState.IN_GAME),\n\
             \tLAST(0xD0, null, ConnectionState.values());",
        )
        .unwrap();
        assert_eq!(c[0].args.len(), 4);
        assert_eq!(c[1].args, ["0xD0", "null", "ConnectionState.values()"]);
    }

    #[test]
    fn null_supplier_and_ex_id() {
        let c = parse("\tREQUEST_SERVER_LOGIN(0x1A, null, ConnectionState.IN_GAME),\n\tX(0x1FF);").unwrap();
        assert_eq!(c[0].args[1], "null");
        assert_eq!(parse_int(&c[1].args[0]).unwrap(), 0x1FF);
    }

    #[test]
    fn server_two_ids_and_comments() {
        let c = parse("\tDIE(0x00), // inline\n\t// ExPackets\n\tEX_DUMMY(0xFE, 0x00);").unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].args, ["0xFE", "0x00"]);
        assert_eq!(c[1].line, 7);
    }

    #[test]
    fn no_argument_constants_and_trailing_members() {
        let src = "public enum E {\n\tA,\n\tB;\n\tprivate int x = foo(1, 2);\n}\n";
        let c = parse_enum(src, "E").unwrap();
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert!(c[0].args.is_empty());
    }

    #[test]
    fn block_comment_inside_list_is_ignored() {
        let c = parse("\t/* old(0x99) */\n\tA(0x01);").unwrap();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn fails_loudly_on_garbage() {
        assert!(parse("\tNOT A CONSTANT;").is_err());
        assert!(parse("\tA(0x01, \"str\");").is_err());
        assert!(parse("\tA(0x01").is_err());
        assert!(parse("\tA(0x01),\n\tB((0x02);").is_err());
        assert!(parse("\tA(0x01),").is_err(), "missing ';' terminator");
        assert!(parse_enum("public enum Other {}", "Sample").is_err());
    }

    #[test]
    fn int_literals() {
        assert_eq!(parse_int("0x0F").unwrap(), 15);
        assert_eq!(parse_int("42").unwrap(), 42);
        assert!(parse_int("0xZZ").is_err());
    }
}
