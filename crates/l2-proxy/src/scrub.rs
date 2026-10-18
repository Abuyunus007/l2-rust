//! Makes a capture safe to commit as a test fixture (M2-CAP-01): the repo is public.
//!
//! - Only `game` channel records are kept. The login channel holds the RSA key, Blowfish keys,
//!   session keys and the announced addresses, and the link channel is never recorded.
//! - Every account name, character name, title and chat line found in the capture (plus any
//!   extra strings given by the caller) is replaced by a placeholder of the **same length**, in
//!   the decoded `fields` and in the raw bytes (UTF-16LE and ASCII). Equal length keeps every
//!   packet layout intact, so byte-level tests still work on the scrubbed file.
//! - Session keys are set to 0: `CharSelectionInfo`/`CharSelected` session ids and the
//!   `KeyPacket` key. `AuthLogin` is cut to its opcode (the proxy already stores it like that).
//! - Afterwards every secret is searched again; any leftover is an error, nothing is written.

use std::collections::BTreeSet;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{CharacterCreate, Say2};
use l2_protocol_game::server::{CharSelected, CharSelectionInfo, UserInfo};
use l2_protocol_game::{ClientOpcode, ServerOpcode};
use serde_json::Value;

use crate::capture::{Channel, Dir, Record};

/// Strings shorter than this are not replaced in raw bytes: two UTF-16 characters are only four
/// bytes and would hit unrelated packet data.
pub const MIN_SECRET_LEN: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum ScrubError {
    #[error("record {index} ({name}) has invalid raw hex")]
    BadHex { index: usize, name: String },
    #[error("{count} secret(s) are still in the scrubbed capture (record {first_record})")]
    Leftover { count: usize, first_record: usize },
}

/// What a scrub did, in counts only (never print the secrets themselves).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrubReport {
    pub kept: usize,
    pub dropped: usize,
    pub secrets: usize,
    pub rewritten: usize,
}

fn body(record: &Record) -> Option<Vec<u8>> {
    let raw = record.raw_bytes().ok()?;
    if raw.len() > 1 { Some(raw) } else { None }
}

fn is(record: &Record, dir: Dir, opcode: u8) -> bool {
    record.channel == Channel::Game && record.dir == dir && record.opcode == opcode
}

/// The personal strings of a capture: account and character names, titles, chat lines.
pub fn collect_secrets(records: &[Record]) -> BTreeSet<String> {
    let mut secrets = BTreeSet::new();
    let mut add = |s: &str| {
        let s = s.trim();
        if !s.is_empty() {
            secrets.insert(s.to_string());
        }
    };
    for r in records {
        let Some(raw) = body(r) else { continue };
        let mut reader = PacketReader::new(&raw[1..]);
        if is(r, Dir::S2c, ServerOpcode::CharacterSelectionInfo.id()) {
            if let Ok(p) = CharSelectionInfo::decode(&mut reader) {
                add(&p.account_name);
                p.characters.iter().for_each(|c| add(&c.name));
            }
        } else if is(r, Dir::S2c, ServerOpcode::CharacterSelected.id()) {
            if let Ok(p) = CharSelected::decode(&mut reader) {
                add(&p.name);
                add(&p.title);
            }
        } else if is(r, Dir::S2c, ServerOpcode::UserInfo.id()) {
            if let Ok(p) = UserInfo::decode(&mut reader) {
                add(&p.name);
                add(&p.title);
            }
        } else if is(r, Dir::C2s, ClientOpcode::CharacterCreate.id()) {
            if let Ok(p) = CharacterCreate::decode(&mut reader) {
                add(&p.name);
            }
        } else if is(r, Dir::C2s, ClientOpcode::Say2.id())
            && let Ok(p) = Say2::decode(&mut reader)
        {
            add(&p.text);
            if let Some(target) = &p.target {
                add(target);
            }
        }
    }
    secrets
}

/// A letters-only placeholder with the same number of UTF-16 units, different per secret.
pub fn placeholder(secret: &str, index: usize) -> String {
    let units = secret.encode_utf16().count();
    let tag = format!("X{}", letters(index));
    let base = format!("{tag}scrubbed");
    base.chars().cycle().take(units).collect()
}

fn letters(mut n: usize) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, char::from(b'a' + (n % 26) as u8));
        n /= 26;
        if n == 0 {
            return out;
        }
        n -= 1;
    }
}

/// Case variants that appear in packets (the client lower-cases account names, titles keep case).
fn variants(secret: &str) -> BTreeSet<String> {
    let mut first_upper = secret.to_lowercase();
    if let Some(c) = first_upper.chars().next() {
        first_upper = c.to_uppercase().collect::<String>() + &first_upper[c.len_utf8()..];
    }
    [
        secret.to_string(),
        secret.to_lowercase(),
        secret.to_uppercase(),
        first_upper,
    ]
    .into_iter()
    .filter(|v| v.encode_utf16().count() == secret.encode_utf16().count())
    .collect()
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn replace_bytes(haystack: &mut [u8], needle: &[u8], with: &[u8]) -> bool {
    debug_assert_eq!(needle.len(), with.len());
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let mut changed = false;
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            haystack[i..i + needle.len()].copy_from_slice(with);
            changed = true;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    changed
}

/// (old, new) string pairs, longest first so a name containing another name is replaced whole.
fn replacement_pairs(secrets: &BTreeSet<String>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (i, secret) in secrets.iter().enumerate() {
        let ph = placeholder(secret, i);
        for v in variants(secret) {
            let ph_v = if v == v.to_lowercase() {
                ph.to_lowercase()
            } else if v == v.to_uppercase() {
                ph.to_uppercase()
            } else {
                ph.clone()
            };
            pairs.push((v, ph_v));
        }
    }
    pairs.sort_by_key(|(old, _)| std::cmp::Reverse(old.len()));
    pairs
}

fn scrub_value(value: &mut Value, pairs: &[(String, String)]) {
    match value {
        Value::String(s) => {
            for (old, new) in pairs {
                if s.contains(old.as_str()) {
                    *s = s.replace(old.as_str(), new);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| scrub_value(v, pairs)),
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                match key.as_str() {
                    "session_id" | "play_ok1" | "play_ok2" | "login_ok1" | "login_ok2" => *v = Value::from(0),
                    "key" => *v = Value::from("0000000000000000"),
                    _ => scrub_value(v, pairs),
                }
            }
        }
        _ => {}
    }
}

/// Re-encodes `raw` after `edit`, but only when the unchanged packet round-trips byte-exact
/// (otherwise the encoder would change more than the edited fields).
fn rewrite<P: Decode + Encode>(raw: &[u8], edit: impl FnOnce(&mut P)) -> Option<Vec<u8>> {
    let mut packet = P::decode(&mut PacketReader::new(&raw[1..])).ok()?;
    if packet.to_bytes() != raw {
        return None;
    }
    edit(&mut packet);
    Some(packet.to_bytes())
}

/// Zeroes the session keys inside known packets.
fn zero_session_keys(record: &Record, raw: &mut Vec<u8>) -> bool {
    let new = if is(record, Dir::S2c, ServerOpcode::CharacterSelectionInfo.id()) {
        rewrite::<CharSelectionInfo>(raw, |p| p.session_id = 0)
    } else if is(record, Dir::S2c, ServerOpcode::CharacterSelected.id()) {
        rewrite::<CharSelected>(raw, |p| p.session_id = 0)
    } else if is(record, Dir::S2c, ServerOpcode::VersionCheck.id()) {
        // result byte, then the 8 key bytes
        let mut out = raw.clone();
        if out.len() >= 10 {
            out[2..10].fill(0);
        }
        Some(out)
    } else if is(record, Dir::C2s, ClientOpcode::AuthLogin.id()) {
        Some(raw[..1].to_vec())
    } else {
        None
    };
    match new {
        Some(new) if new != *raw => {
            *raw = new;
            true
        }
        _ => false,
    }
}

/// Scrubs `records`. `extra` adds strings the capture itself does not reveal (clan names, ...).
pub fn scrub(records: Vec<Record>, extra: &[String]) -> Result<(Vec<Record>, ScrubReport), ScrubError> {
    let mut report = ScrubReport::default();
    let mut secrets = collect_secrets(&records);
    secrets.extend(
        extra
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    );
    report.secrets = secrets.len();
    let pairs = replacement_pairs(&secrets);
    let raw_pairs: Vec<(Vec<u8>, Vec<u8>)> = pairs
        .iter()
        .filter(|(old, _)| old.encode_utf16().count() >= MIN_SECRET_LEN)
        .flat_map(|(old, new)| {
            [
                (utf16(old), utf16(new)),
                (old.as_bytes().to_vec(), new.as_bytes().to_vec()),
            ]
        })
        .filter(|(old, new)| old.len() == new.len())
        .collect();

    let mut out = Vec::with_capacity(records.len());
    for (index, mut record) in records.into_iter().enumerate() {
        if record.channel != Channel::Game {
            report.dropped += 1;
            continue;
        }
        let mut raw = record.raw_bytes().map_err(|_| ScrubError::BadHex {
            index,
            name: record.name.clone(),
        })?;
        let mut changed = zero_session_keys(&record, &mut raw);
        for (old, new) in &raw_pairs {
            changed |= replace_bytes(&mut raw, old, new);
        }
        if changed {
            report.rewritten += 1;
        }
        record.raw = hex::encode(&raw);
        if let Some(fields) = &mut record.fields {
            scrub_value(fields, &pairs);
        }
        out.push(record);
    }
    report.kept = out.len();
    check_leftovers(&out, &secrets)?;
    Ok((out, report))
}

/// Fails if a secret of [`MIN_SECRET_LEN`] or more units is still anywhere in the records.
pub fn check_leftovers(records: &[Record], secrets: &BTreeSet<String>) -> Result<(), ScrubError> {
    let mut count = 0;
    let mut first_record = usize::MAX;
    for (i, r) in records.iter().enumerate() {
        let raw = r.raw_bytes().unwrap_or_default();
        let fields = r
            .fields
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default()
            .to_lowercase();
        for s in secrets
            .iter()
            .filter(|s| s.encode_utf16().count() >= MIN_SECRET_LEN)
        {
            let in_raw = variants(s)
                .iter()
                .any(|v| contains(&raw, &utf16(v)) || contains(&raw, v.as_bytes()));
            if in_raw || fields.contains(&s.to_lowercase()) {
                count += 1;
                first_record = first_record.min(i);
            }
        }
    }
    if count == 0 {
        Ok(())
    } else {
        Err(ScrubError::Leftover { count, first_record })
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use l2_protocol_game::server::charscreen::CharSelectEntry;

    use super::*;

    fn game(dir: Dir, raw: Vec<u8>, fields: Option<Value>) -> Record {
        Record::new(1, dir, Channel::Game, raw[0], "X", fields, &raw)
    }

    fn char_list() -> Vec<u8> {
        CharSelectionInfo {
            account_name: "myaccount".into(),
            session_id: 0x1234_5678,
            max_characters: 7,
            characters: vec![CharSelectEntry {
                name: "Herobrine".into(),
                object_id: 0x1000_0001,
                ..CharSelectEntry::default()
            }],
        }
        .to_bytes()
    }

    #[test]
    fn placeholders_keep_the_length_and_differ() {
        assert_eq!(placeholder("Herobrine", 0).encode_utf16().count(), 9);
        assert_ne!(placeholder("abcdef", 0), placeholder("abcdef", 1));
        assert!(placeholder("abc", 30).chars().all(|c| c.is_ascii_alphabetic()));
    }

    #[test]
    fn names_and_session_keys_are_removed_from_raw_and_fields() {
        let raw = char_list();
        let fields = serde_json::json!({"account_name": "myaccount", "session_id": 305_419_896,
            "characters": [{"name": "Herobrine"}]});
        let chat = Say2 {
            text: "my secret words".into(),
            chat_type: 0,
            target: None,
        }
        .to_bytes();
        let echo = {
            // The server's echo of the chat line, as CreatureSay would carry it.
            let mut b = vec![ServerOpcode::Say2.id()];
            b.extend(utf16("Herobrine"));
            b.extend(utf16("my secret words"));
            b
        };
        let login = Record::new(9, Dir::S2c, Channel::Login, 3, "LoginOk", None, &[3, 1, 2]);
        let records = vec![
            login,
            game(Dir::S2c, raw.clone(), Some(fields)),
            game(Dir::C2s, chat, None),
            game(Dir::S2c, echo, None),
        ];
        let (out, report) = scrub(records, &[]).unwrap();
        assert_eq!((report.kept, report.dropped), (3, 1));
        assert_eq!(report.secrets, 3);

        let scrubbed = out[0].raw_bytes().unwrap();
        assert_eq!(scrubbed.len(), raw.len(), "layout kept");
        let list = CharSelectionInfo::decode(&mut PacketReader::new(&scrubbed[1..])).unwrap();
        assert_eq!(list.session_id, 0);
        assert_ne!(list.account_name, "myaccount");
        assert_eq!(list.characters[0].name.len(), "Herobrine".len());
        assert_eq!(list.characters[0].object_id, 0x1000_0001);
        let text = out[0].fields.as_ref().unwrap().to_string();
        assert!(!text.contains("myaccount") && !text.contains("Herobrine"));
        assert!(text.contains("\"session_id\":0"));
        // The chat line is gone from both directions.
        for r in &out[1..] {
            assert!(!contains(&r.raw_bytes().unwrap(), &utf16("my secret words")));
        }
    }

    #[test]
    fn the_key_and_auth_login_are_cleared() {
        let mut key = vec![
            ServerOpcode::VersionCheck.id(),
            1,
            9,
            9,
            9,
            9,
            9,
            9,
            9,
            9,
            0,
            0,
            0,
            0,
        ];
        key.extend([2, 0, 0, 0, 1, 0, 0, 0, 0]);
        let auth = vec![ClientOpcode::AuthLogin.id(), 0x61, 0, 0, 0];
        let (out, _) = scrub(vec![game(Dir::S2c, key, None), game(Dir::C2s, auth, None)], &[]).unwrap();
        assert_eq!(&out[0].raw_bytes().unwrap()[2..10], &[0; 8]);
        assert_eq!(out[1].raw_bytes().unwrap(), [ClientOpcode::AuthLogin.id()]);
    }

    #[test]
    fn extra_secrets_are_replaced_and_leftovers_are_detected() {
        let mut raw = vec![0x99];
        raw.extend(utf16("ClanOfMine"));
        let (out, _) = scrub(vec![game(Dir::S2c, raw.clone(), None)], &["ClanOfMine".into()]).unwrap();
        assert!(!contains(&out[0].raw_bytes().unwrap(), &utf16("ClanOfMine")));

        let unscrubbed = vec![game(Dir::S2c, raw, None)];
        let secrets: BTreeSet<String> = ["ClanOfMine".to_string()].into();
        assert!(matches!(
            check_leftovers(&unscrubbed, &secrets),
            Err(ScrubError::Leftover { count: 1, .. })
        ));
    }
}
