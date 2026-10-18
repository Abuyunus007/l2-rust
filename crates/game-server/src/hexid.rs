//! `config/hexid.txt`: the id and secret this game server uses to authenticate at the login
//! server.
// Java: org/l2jmobius/gameserver/config/ServerConfig.java (loadHexid, saveHexid)
// Java: org/l2jmobius/gameserver/LoginServerThread.java (constructor, AuthResponse)
//
// The Rust game server runs with its own config folder (D-39), so this file is never the Java
// game server's `hexid.txt`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use l2_config::IniFile;
use tracing::{info, warn};

/// File name inside the config directory (Java `./config/hexid.txt`).
pub const HEXID_FILE: &str = "hexid.txt";

/// What a valid `hexid.txt` holds.
#[derive(Clone, PartialEq, Eq)]
pub struct HexId {
    /// `ServerID`: the id the login server assigned.
    pub server_id: i32,
    /// `HexID`, as Java's `new BigInteger(text, 16).toByteArray()` (signed big-endian bytes).
    /// This is what `AuthRequest` sends.
    pub hex_id: Vec<u8>,
}

// The hex id is the secret of the registration, so it stays out of logs.
impl std::fmt::Debug for HexId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HexId")
            .field("server_id", &self.server_id)
            .field("hex_id", &"<redacted>")
            .finish()
    }
}

/// Reads `hexid.txt` from `config_dir`. `None` when the file is missing or unusable: the game
/// server then asks the login server for a registration (`RequestServerID`) and hopes it is
/// given one, like Java ("Hopefully login will give us one").
// Java: ServerConfig.loadHexid
pub fn load(config_dir: &Path) -> Option<HexId> {
    let path = config_dir.join(HEXID_FILE);
    if !path.exists() {
        // Java stays silent here when the file is missing, but then logs the line below because
        // `HEX_ID` is still null. One message is enough.
        warn!(
            "Could not load HexID file ({}). Hopefully login will give us one.",
            path.display()
        );
        return None;
    }
    let ini = match IniFile::load(&path) {
        Ok(ini) => ini,
        Err(e) => {
            warn!("Could not read {}: {e}", path.display());
            return None;
        }
    };
    let hexid = parse(&ini);
    if hexid.is_none() {
        warn!(
            "Could not load HexID file ({}). Hopefully login will give us one.",
            path.display()
        );
    }
    hexid
}

/// The checks of `loadHexid` on an already parsed file.
fn parse(ini: &IniFile) -> Option<HexId> {
    if !(ini.contains_key("ServerID") && ini.contains_key("HexID")) {
        return None;
    }
    // Java: `SERVER_ID = getInt("ServerID", 1)`. NOTE(java-quirk): it is assigned before the
    // HexID is parsed, but it only matters together with a valid HexID.
    let server_id = ini.get_int("ServerID", 1);
    // Java: `new BigInteger(hexId.getString("HexID", null), 16).toByteArray()`; any failure
    // (not hex, empty) leaves `HEX_ID` null.
    let text = ini.get_string("HexID", "");
    let hex_id = l2_db::string_to_hex(&text).ok()?;
    Some(HexId { server_id, hex_id })
}

/// Writes `hexid.txt` after the login server accepted the registration. `hex_id` is the text
/// form of Java's `hexToString` ([`l2_db::hex_to_string`]).
///
/// NOTE(java-quirk): Java only writes when the file does not exist yet. A registration under a
/// different id never overwrites an existing file; the old content keeps being used.
// Java: ServerConfig.saveHexid
pub fn save(config_dir: &Path, server_id: i32, hex_id: &str) {
    let path = config_dir.join(HEXID_FILE);
    // `create_new` makes "only if absent" atomic.
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return,
        Err(e) => {
            warn!("Failed to save hex id to {} File.", path.display());
            warn!("Config: {e}");
            return;
        }
    };
    // Java: `Properties.store(out, "The HexId to Auth into LoginServer")` (the date line it
    // adds is only a comment).
    let text = format!("#The HexId to Auth into LoginServer\nHexID={hex_id}\nServerID={server_id}\n");
    match file.write_all(text.as_bytes()) {
        Ok(()) => info!("Gameserver: Generated new HexID file for server id {server_id}."),
        Err(e) => {
            warn!("Failed to save hex id to {} File.", path.display());
            warn!("Config: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "-2ad66b3f483c22be097019f55c8abdf0";

    fn ini(text: &str) -> IniFile {
        IniFile::parse(HEXID_FILE, text).unwrap()
    }

    #[test]
    fn parses_both_keys_like_java() {
        let hexid = parse(&ini("HexID=-2ad66b3f483c22be097019f55c8abdf0\nServerID=7\n")).unwrap();
        assert_eq!(hexid.server_id, 7);
        // BigInteger.toByteArray of a negative 127-bit number: 16 bytes, sign bit set.
        assert_eq!(hexid.hex_id, l2_db::string_to_hex(SAMPLE).unwrap());
        assert_eq!(hexid.hex_id.len(), 16);
        assert_eq!(l2_db::hex_to_string(&hexid.hex_id).unwrap(), SAMPLE);
    }

    #[test]
    fn the_java_header_lines_are_comments() {
        let text = "#the hexID to auth into login\n#Mon May 16 21:56:15 EEST 2016\nHexID=ff\nServerID=2\n";
        let hexid = parse(&ini(text)).unwrap();
        assert_eq!(hexid.server_id, 2);
        // 0xff is positive for BigInteger, so it needs a leading zero byte.
        assert_eq!(hexid.hex_id, vec![0x00, 0xff]);
    }

    #[test]
    fn spaces_around_the_equals_sign_are_accepted() {
        let hexid = parse(&ini("HexID = 1a\nServerID = 3\n")).unwrap();
        assert_eq!((hexid.server_id, hexid.hex_id), (3, vec![0x1a]));
    }

    #[test]
    fn both_keys_are_required() {
        assert_eq!(parse(&ini("HexID=1a\n")), None);
        assert_eq!(parse(&ini("ServerID=1\n")), None);
        assert_eq!(parse(&ini("")), None);
    }

    #[test]
    fn an_invalid_hex_id_is_rejected() {
        assert_eq!(parse(&ini("HexID=xyz\nServerID=1\n")), None);
        assert_eq!(parse(&ini("HexID=\nServerID=1\n")), None);
    }

    #[test]
    fn a_bad_server_id_falls_back_to_one() {
        // Java getInt: an unparsable value gives the default.
        let hexid = parse(&ini("HexID=1a\nServerID=abc\n")).unwrap();
        assert_eq!(hexid.server_id, 1);
    }

    #[test]
    fn debug_output_hides_the_secret() {
        let hexid = HexId {
            server_id: 2,
            hex_id: vec![0xde, 0xad],
        };
        let shown = format!("{hexid:?}");
        assert!(
            !shown.contains("222") && !shown.to_lowercase().contains("dead"),
            "{shown}"
        );
        assert!(shown.contains("server_id: 2"));
    }

    #[test]
    fn missing_file_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), 5, SAMPLE);
        let text = std::fs::read_to_string(dir.path().join(HEXID_FILE)).unwrap();
        assert!(
            text.starts_with("#The HexId to Auth into LoginServer\n"),
            "{text}"
        );
        let hexid = load(dir.path()).unwrap();
        assert_eq!(hexid.server_id, 5);
        assert_eq!(l2_db::hex_to_string(&hexid.hex_id).unwrap(), SAMPLE);
    }

    #[test]
    fn save_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), 5, "1a");
        save(dir.path(), 6, "2b");
        let hexid = load(dir.path()).unwrap();
        assert_eq!((hexid.server_id, hexid.hex_id), (5, vec![0x1a]));
    }

    #[test]
    fn unusable_file_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(HEXID_FILE), "HexID=nothex\nServerID=4\n").unwrap();
        assert_eq!(load(dir.path()), None);
    }
}
