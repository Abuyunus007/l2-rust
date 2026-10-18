//! Target `opcodes`: game packet opcode tables.
//!
//! Java: gameserver/network/{ClientPackets,ExClientPackets,ServerPackets,ConnectionState}.java
//! Output: crates/l2-protocol-game/src/generated/opcodes.rs (+ generated/mod.rs)
//!
//! This is the reference generator. The shape every generator follows:
//! 1. `read_sources` loads the Java files (the only I/O).
//! 2. `generate` is a pure function from source text to [`Output`] files, so it is unit-testable
//!    with small samples and deterministic.
//! 3. Tables are parsed with [`crate::java_enum`], validated (unique ids, known names), then turned
//!    into tokens with `quote` and formatted by `prettyplease`.

use super::{Output, game_generated_mod, render};
use crate::java_enum::{Constant, parse_enum, parse_int};
use anyhow::{Context, Result, anyhow, bail};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const NETWORK_DIR: &str = "java/org/l2jmobius/gameserver/network";

/// The Java source text of the four files this target reads.
pub struct Sources {
    pub connection_state: String,
    pub client: String,
    pub ex_client: String,
    pub server: String,
}

pub fn read_sources(l2j_root: &Path) -> Result<Sources> {
    let read = |file: &str| -> Result<String> {
        let path = l2j_root.join(NETWORK_DIR).join(file);
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    };
    Ok(Sources {
        connection_state: read("ConnectionState.java")?,
        client: read("ClientPackets.java")?,
        ex_client: read("ExClientPackets.java")?,
        server: read("ServerPackets.java")?,
    })
}

/// `LOGOUT` -> `Logout`, `REQUEST_EX_2ND_PASSWORD_CHECK` -> `RequestEx2ndPasswordCheck`.
fn camel_case(java_name: &str) -> String {
    java_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase());
            first
                .into_iter()
                .chain(chars.map(|c| c.to_ascii_lowercase()))
                .collect::<String>()
        })
        .collect()
}

struct Variant {
    java_name: String,
    rust_name: String,
}

/// Converts Java names to unique Rust variant names (fails on a collision).
fn variants(constants: &[Constant]) -> Result<Vec<Variant>> {
    let mut seen: HashMap<String, &str> = HashMap::new();
    let mut out = Vec::new();
    for c in constants {
        let rust_name = camel_case(&c.name);
        if rust_name.is_empty() || rust_name.starts_with(|ch: char| ch.is_ascii_digit()) {
            bail!("{}: cannot make a Rust variant name out of it", c.name);
        }
        if let Some(other) = seen.insert(rust_name.clone(), &c.name) {
            bail!("{} and {other} both become `{rust_name}`", c.name);
        }
        out.push(Variant {
            java_name: c.name.clone(),
            rust_name,
        });
    }
    Ok(out)
}

/// A client table row, with `ConnectionState`s as indexes into the `ConnectionState` enum.
struct ClientRow {
    variant: Variant,
    id: u32,
    states: Vec<usize>,
}

fn client_rows(
    table: &str,
    constants: &[Constant],
    states: &[Variant],
    max_id: u32,
) -> Result<Vec<ClientRow>> {
    let vars = variants(constants)?;
    let mut ids: BTreeMap<u32, &str> = BTreeMap::new();
    let mut rows = Vec::new();
    for (c, variant) in constants.iter().zip(vars) {
        let err = || format!("{table}.{} (line {})", c.name, c.line);
        // NAME(id, supplier, state...). The supplier (`Foo::new` or `null`) is not needed here.
        if c.args.len() < 3 {
            bail!("{}: expected `id, supplier, state...`, got {:?}", err(), c.args);
        }
        let id = parse_int(&c.args[0]).with_context(err)?;
        if id > max_id {
            bail!("{}: id {id:#x} is bigger than {max_id:#x}", err());
        }
        if let Some(other) = ids.insert(id, &c.name) {
            bail!("{}: id {id:#x} is already used by {other}", err());
        }
        // Java stores the states in an EnumSet: duplicates collapse, order is the enum's order.
        let mut set = vec![false; states.len()];
        for arg in &c.args[2..] {
            if arg == "ConnectionState.values()" {
                set.iter_mut().for_each(|s| *s = true);
            } else {
                let name = arg
                    .strip_prefix("ConnectionState.")
                    .ok_or_else(|| anyhow!("{}: `{arg}` is not a ConnectionState", err()))?;
                let index = states
                    .iter()
                    .position(|s| s.java_name == name)
                    .ok_or_else(|| anyhow!("{}: unknown ConnectionState `{name}`", err()))?;
                set[index] = true;
            }
        }
        let states = (0..set.len()).filter(|&i| set[i]).collect();
        rows.push(ClientRow { variant, id, states });
    }
    Ok(rows)
}

struct ServerRow {
    variant: Variant,
    id: u32,
    ex_id: Option<u32>,
}

fn server_rows(constants: &[Constant]) -> Result<Vec<ServerRow>> {
    let vars = variants(constants)?;
    let mut seen: BTreeMap<(u32, Option<u32>), &str> = BTreeMap::new();
    let mut rows = Vec::new();
    for (c, variant) in constants.iter().zip(vars) {
        let err = || format!("ServerPackets.{} (line {})", c.name, c.line);
        let (id, ex_id) = match c.args.as_slice() {
            [id] => (parse_int(id).with_context(err)?, None),
            [id, ex] => (
                parse_int(id).with_context(err)?,
                Some(parse_int(ex).with_context(err)?),
            ),
            other => bail!("{}: expected `id` or `id, exId`, got {other:?}", err()),
        };
        if id > 0xFF || ex_id.is_some_and(|e| e > 0xFFFF) {
            bail!("{}: id out of range (u8 / u16)", err());
        }
        if let Some(other) = seen.insert((id, ex_id), &c.name) {
            bail!("{}: ids ({id:#x}, {ex_id:?}) are already used by {other}", err());
        }
        rows.push(ServerRow { variant, id, ex_id });
    }
    Ok(rows)
}

fn lit(value: u32, width: usize) -> Result<syn::LitInt> {
    Ok(syn::parse_str(&format!("{value:#0width$X}", width = width + 2))?)
}

/// Pure generator: Java text in, files out.
pub fn generate(src: &Sources) -> Result<Vec<Output>> {
    let state_consts = parse_enum(&src.connection_state, "ConnectionState")?;
    if state_consts.iter().any(|c| !c.args.is_empty()) {
        bail!("ConnectionState: constants with arguments are not supported");
    }
    let states = variants(&state_consts)?;
    let client = client_rows(
        "ClientPackets",
        &parse_enum(&src.client, "ClientPackets")?,
        &states,
        0xFF,
    )?;
    let ex_client = client_rows(
        "ExClientPackets",
        &parse_enum(&src.ex_client, "ExClientPackets")?,
        &states,
        0xFFFF,
    )?;
    let server = server_rows(&parse_enum(&src.server, "ServerPackets")?)?;

    let state_names: Vec<_> = states.iter().map(|s| format_ident!("{}", s.rust_name)).collect();
    let state_java: Vec<_> = states.iter().map(|s| s.java_name.as_str()).collect();

    let connection_state = quote! {
        /// Connection state of a game client. A packet is only accepted in the states listed for it.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum ConnectionState {
            #(#state_names,)*
        }

        impl ConnectionState {
            /// Every state, in Java declaration order.
            pub const ALL: &'static [Self] = &[#(Self::#state_names,)*];

            /// The Java constant name, e.g. `IN_GAME`.
            pub const fn name(self) -> &'static str {
                match self {
                    #(Self::#state_names => #state_java,)*
                }
            }
        }
    };

    let client_tokens = client_enum(
        "ClientOpcode",
        "Client packet ids (first byte of a client packet). `ExPacket` (0xD0) is followed by a `u16` [`ExClientOpcode`].",
        "u8",
        2,
        &client,
        &states,
    )?;
    let ex_client_tokens = client_enum(
        "ExClientOpcode",
        "Client ex packet ids: the `u16` that follows the 0xD0 byte.",
        "u16",
        4,
        &ex_client,
        &states,
    )?;
    let server_tokens = server_enum(&server)?;

    let tokens = quote! {
        #connection_state
        #client_tokens
        #ex_client_tokens
        #server_tokens
    };
    let note = "// Source: Java gameserver/network/{ConnectionState,ClientPackets,ExClientPackets,ServerPackets}.java\n\
                // Regenerate: cargo run -p codegen -- opcodes";
    Ok(vec![
        game_generated_mod()?,
        Output {
            path: "crates/l2-protocol-game/src/generated/opcodes.rs".into(),
            contents: render(note, tokens)?,
        },
    ])
}

fn client_enum(
    name: &str,
    doc: &str,
    repr: &str,
    hex_width: usize,
    rows: &[ClientRow],
    states: &[Variant],
) -> Result<impl quote::ToTokens> {
    let enum_name = format_ident!("{name}");
    let doc = format!(" {doc}"); // `///` comments start with a space
    let repr = format_ident!("{repr}");
    let variants: Vec<_> = rows
        .iter()
        .map(|r| format_ident!("{}", r.variant.rust_name))
        .collect();
    let java: Vec<_> = rows.iter().map(|r| r.variant.java_name.as_str()).collect();
    let ids = rows
        .iter()
        .map(|r| lit(r.id, hex_width))
        .collect::<Result<Vec<_>>>()?;
    let count = rows.len();

    // Group variants with identical state lists, so the table stays readable.
    let mut groups: Vec<(&Vec<usize>, Vec<&syn::Ident>)> = Vec::new();
    for (row, variant) in rows.iter().zip(&variants) {
        match groups.iter_mut().find(|(s, _)| **s == row.states) {
            Some((_, members)) => members.push(variant),
            None => groups.push((&row.states, vec![variant])),
        }
    }
    let group_arms = groups.iter().map(|(set, members)| {
        let names = set.iter().map(|&i| format_ident!("{}", states[i].rust_name));
        quote! { #(Self::#members)|* => &[#(ConnectionState::#names),*] }
    });

    Ok(quote! {
        #[doc = #doc]
        #[repr(#repr)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum #enum_name {
            #(#variants = #ids,)*
        }

        impl #enum_name {
            /// Every opcode, in Java declaration order.
            pub const ALL: [Self; #count] = [#(Self::#variants,)*];

            /// The numeric id.
            pub const fn id(self) -> #repr {
                self as #repr
            }

            /// The Java constant name, e.g. `REQUEST_ITEM_LIST`.
            pub const fn name(self) -> &'static str {
                match self {
                    #(Self::#variants => #java,)*
                }
            }

            pub const fn from_id(id: #repr) -> Option<Self> {
                match id {
                    #(#ids => Some(Self::#variants),)*
                    _ => None,
                }
            }

            /// The connection states in which the Java server accepts this packet.
            pub const fn connection_states(self) -> &'static [ConnectionState] {
                match self {
                    #(#group_arms,)*
                }
            }
        }
    })
}

fn server_enum(rows: &[ServerRow]) -> Result<impl quote::ToTokens> {
    let variants: Vec<_> = rows
        .iter()
        .map(|r| format_ident!("{}", r.variant.rust_name))
        .collect();
    let java: Vec<_> = rows.iter().map(|r| r.variant.java_name.as_str()).collect();
    let count = rows.len();
    let ids = rows.iter().map(|r| lit(r.id, 2)).collect::<Result<Vec<_>>>()?;
    let ex_ids = rows
        .iter()
        .map(|r| match r.ex_id {
            Some(ex) => lit(ex, 2).map(|l| quote! { Some(#l) }),
            None => Ok(quote! { None }),
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        /// Server packet ids. Plain packets have one id byte. Ex packets are written as the
        /// byte `0xFE` followed by a `u16` ex id (`ex_id()`).
        ///
        /// NOTE(java-quirk): Java writes the ex id only when it is greater than 0, so `ExDummy`
        /// (0xFE, 0x00) goes on the wire as the single byte 0xFE. The table keeps the declared `Some(0)`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum ServerOpcode {
            #(#variants,)*
        }

        impl ServerOpcode {
            /// Every opcode, in Java declaration order.
            pub const ALL: [Self; #count] = [#(Self::#variants,)*];

            /// The first id byte (0xFE for ex packets).
            pub const fn id(self) -> u8 {
                match self {
                    #(Self::#variants => #ids,)*
                }
            }

            /// The second id (`u16`), for ex packets only.
            pub const fn ex_id(self) -> Option<u16> {
                match self {
                    #(Self::#variants => #ex_ids,)*
                }
            }

            /// The Java constant name, e.g. `CHARACTER_SELECTION_INFO`.
            pub const fn name(self) -> &'static str {
                match self {
                    #(Self::#variants => #java,)*
                }
            }

            /// Looks an opcode up by the id byte and, for ex packets, the `u16` ex id.
            pub const fn from_id(id: u8, ex_id: Option<u16>) -> Option<Self> {
                match (id, ex_id) {
                    #((#ids, #ex_ids) => Some(Self::#variants),)*
                    _ => None,
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Sources {
        let wrap = |name: &str, body: &str| format!("package x;\npublic enum {name}\n{{\n{body}\n}}\n");
        Sources {
            connection_state: wrap("ConnectionState", "\tCONNECTED,\n\tAUTHENTICATED,\n\tIN_GAME;"),
            client: wrap(
                "ClientPackets",
                "\tLOGOUT(0x00, Logout::new, ConnectionState.AUTHENTICATED, ConnectionState.IN_GAME),\n\
                 \tATTACK(0x01, AttackRequest::new, ConnectionState.IN_GAME),\n\
                 \tUSE_ITEM(0x19, UseItem::new, ConnectionState.IN_GAME),\n\
                 \tEX_PACKET(0xD0, null, ConnectionState.values());",
            ),
            ex_client: wrap(
                "ExClientPackets",
                "\tREQUEST_GOTO_LOBBY(0x36, RequestGotoLobby::new, ConnectionState.AUTHENTICATED),\n\
                 \tREQUEST_SERVER_LOGIN(0x1A, null, ConnectionState.IN_GAME);",
            ),
            server: wrap(
                "ServerPackets",
                "\tDIE(0x00),\n\tEX_DUMMY(0xFE, 0x00),\n\tEX_REGEN_MAX(0xFE, 0x01);",
            ),
        }
    }

    fn opcodes_rs(src: &Sources) -> String {
        let out = generate(src).unwrap();
        out.into_iter()
            .find(|o| o.path.ends_with("opcodes.rs"))
            .unwrap()
            .contents
    }

    #[test]
    fn camel_case_names() {
        assert_eq!(camel_case("LOGOUT"), "Logout");
        assert_eq!(
            camel_case("REQUEST_EX_2ND_PASSWORD_CHECK"),
            "RequestEx2ndPasswordCheck"
        );
        assert_eq!(camel_case("IN_GAME"), "InGame");
    }

    #[test]
    fn output_is_deterministic_and_has_header() {
        let a = generate(&sample()).unwrap();
        let b = generate(&sample()).unwrap();
        assert_eq!(a, b);
        assert!(a.iter().all(|o| o.contents.starts_with(super::super::HEADER)));
    }

    #[test]
    fn generated_content_has_expected_pieces() {
        // Compare without whitespace: prettyplease wraps long lines.
        let text: String = opcodes_rs(&sample()).split_whitespace().collect();
        let has = |needle: &str| text.contains(&needle.split_whitespace().collect::<String>());
        assert!(has("Logout = 0x00,"), "{text}");
        assert!(has("ExPacket = 0xD0,"));
        assert!(has("RequestServerLogin = 0x001A,"));
        assert!(has("(0xFE, Some(0x01)) => Some(Self::ExRegenMax)"));
        assert!(has("(0x00, None) => Some(Self::Die)"));
        // Logout: two states, in ConnectionState declaration order.
        assert!(has("&[ConnectionState::Authenticated, ConnectionState::InGame]"));
        // `values()` expands to every state.
        // (no closing bracket: a wrapped list has a trailing comma)
        assert!(has(
            "&[ConnectionState::Connected, ConnectionState::Authenticated, ConnectionState::InGame"
        ));
    }

    #[test]
    fn duplicate_state_collapses_like_enum_set() {
        let mut src = sample();
        src.ex_client = src.ex_client.replace(
            "ConnectionState.AUTHENTICATED)",
            "ConnectionState.AUTHENTICATED, ConnectionState.AUTHENTICATED)",
        );
        assert!(generate(&src).is_ok());
    }

    #[test]
    fn rejects_duplicate_ids_unknown_states_and_name_clashes() {
        let mut src = sample();
        src.client = src.client.replace("0x19", "0x01");
        assert!(generate(&src).unwrap_err().to_string().contains("already used"));

        let mut src = sample();
        src.client = src.client.replace(
            "ConnectionState.IN_GAME),\n\tUSE",
            "ConnectionState.NOPE),\n\tUSE",
        );
        assert!(format!("{:#}", generate(&src).unwrap_err()).contains("unknown ConnectionState"));

        let mut src = sample();
        src.server = src.server.replace("DIE(0x00)", "EX_DUMMY(0x05)");
        assert!(generate(&src).is_err());

        let mut src = sample();
        src.client = src.client.replace("0x19", "0x1FF");
        assert!(generate(&src).is_err(), "client id must fit in a byte");
    }

    #[test]
    fn rejects_malformed_rows() {
        let mut src = sample();
        src.client = src.client.replace(
            "USE_ITEM(0x19, UseItem::new, ConnectionState.IN_GAME)",
            "USE_ITEM(0x19)",
        );
        assert!(generate(&src).is_err());
        let mut src = sample();
        src.server = src.server.replace("DIE(0x00)", "DIE(0x00, 0x01, 0x02)");
        assert!(generate(&src).is_err());
    }
}
