//! Banned IP addresses: the in-memory ban list and the `banned_ip.cfg` loader.
// Java: org/l2jmobius/loginserver/LoginServer.java (`loadBanFile`, `processBanLine`)
// Java: org/l2jmobius/loginserver/controller/LoginController.java
//       (`addBanForAddress`, `isBannedAddress`, `removeBanForAddress`, `_bannedIps`)
//
// File format, one entry per line: `ip [durationMillis]  # comment`. The ip is any text; a
// subnet is written with trailing zero groups (`123.45.0.0`) and matched by `is_banned`.

use std::path::Path;

use dashmap::DashMap;
use l2_util::time::now_millis;
use tracing::{info, warn};

/// Expiry value of a ban without duration (`Long.MAX_VALUE`).
pub const PERMANENT_BAN: i64 = i64::MAX;

/// Address -> expiry in milliseconds since the epoch ([`PERMANENT_BAN`] for no expiry).
///
/// Shared between connection tasks, so it is a concurrent map (Java: `ConcurrentHashMap`).
#[derive(Debug, Default)]
pub struct BanList {
    bans: DashMap<String, i64>,
}

impl BanList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bans `address` for `duration_ms`, or permanently when `duration_ms <= 0`. An address that
    /// is already banned keeps its old expiry (Java `putIfAbsent`).
    // Java: LoginController.addBanForAddress
    pub fn add(&self, address: &str, duration_ms: i64) {
        self.add_at(address, duration_ms, now_millis());
    }

    /// [`Self::add`] with an explicit clock, for tests.
    pub fn add_at(&self, address: &str, duration_ms: i64, now_ms: i64) {
        let expires = if duration_ms > 0 {
            // NOTE(java-quirk): `System.currentTimeMillis() + duration` wraps on overflow. A
            // wrapped (negative) expiry is never treated as expired, so it acts as permanent.
            now_ms.wrapping_add(duration_ms)
        } else {
            PERMANENT_BAN
        };
        self.bans.entry(address.to_owned()).or_insert(expires);
    }

    /// True while `address` or one of its subnets (`a.b.c.0`, `a.b.0.0`, `a.0.0.0`) is banned.
    /// An expired exact ban is removed and reported as not banned.
    // Java: LoginController.isBannedAddress
    pub fn is_banned(&self, address: &str) -> bool {
        self.is_banned_at(address, now_millis())
    }

    /// [`Self::is_banned`] with an explicit clock, for tests.
    pub fn is_banned_at(&self, address: &str, now_ms: i64) -> bool {
        // `get` returns a guard into the map; copy the value out so the guard is gone before
        // any `remove` below (DashMap deadlocks otherwise).
        let lookup = |key: &str| self.bans.get(key).map(|expiry| *expiry);

        let mut found = lookup(address);
        if found.is_none() {
            let parts = split_address(address);
            // NOTE(java-quirk): Java indexes `parts[2]` unguarded, so an address with fewer than
            // three dot-separated groups (an IPv6 literal) that is not banned exactly throws
            // there. The Rust version reports "not banned" instead of failing.
            if parts.len() < 3 {
                return false;
            }
            found = lookup(&format!("{}.{}.{}.0", parts[0], parts[1], parts[2]));
            if found.is_none() {
                found = lookup(&format!("{}.{}.0.0", parts[0], parts[1]));
            }
            if found.is_none() {
                found = lookup(&format!("{}.0.0.0", parts[0]));
            }
        }

        let Some(expiry) = found else {
            return false;
        };
        if expiry > 0 && expiry < now_ms {
            // NOTE(java-quirk): Java removes `address` (the one asked about), not the key that
            // matched. For an expired subnet ban nothing is removed, the entry stays in the
            // list, and every lookup keeps answering "not banned" until the server restarts.
            self.bans.remove(address);
            info!("Removed expired ip address ban {address}.");
            return false;
        }
        true
    }

    /// Removes the ban of exactly this address. True when there was one.
    // Java: LoginController.removeBanForAddress
    pub fn remove(&self, address: &str) -> bool {
        self.bans.remove(address).is_some()
    }

    /// Expiry of the ban stored under exactly this key.
    pub fn expiry(&self, address: &str) -> Option<i64> {
        self.bans.get(address).map(|expiry| *expiry)
    }

    pub fn len(&self) -> usize {
        self.bans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bans.is_empty()
    }
}

/// Java `address.split("\\.")`: trailing empty parts are dropped.
fn split_address(address: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = address.split('.').collect();
    while parts.len() > 1 && parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

/// What one line of `banned_ip.cfg` means.
#[derive(Debug, PartialEq, Eq)]
pub enum BanLine<'a> {
    /// Empty line or comment.
    Ignore,
    /// A ban to register; `duration_ms` is 0 when the line has no duration.
    Entry { address: &'a str, duration_ms: i64 },
    /// A line Java skips with a warning; the text says why.
    Invalid(String),
}

/// Parses one line of `banned_ip.cfg`.
// Java: LoginServer.processBanLine
pub fn parse_ban_line(raw: &str) -> BanLine<'_> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return BanLine::Ignore;
    }
    let definition = line.find('#').map_or(line, |index| line[..index].trim());
    if definition.is_empty() {
        return BanLine::Invalid("empty definition".to_owned());
    }
    let mut tokens = definition.split_whitespace();
    let Some(address) = tokens.next() else {
        return BanLine::Invalid("no address token".to_owned());
    };
    let mut duration_ms = 0;
    if let Some(token) = tokens.next() {
        match token.parse::<i64>() {
            Ok(value) => duration_ms = value,
            Err(_) => return BanLine::Invalid(format!("invalid duration token '{token}'")),
        }
    }
    // Further tokens are ignored, like Java.
    BanLine::Entry { address, duration_ms }
}

/// Registers every entry of the ban file `path` in `bans`. A missing file only logs a warning.
/// Returns the number of bans in the list afterwards.
// Java: LoginServer.loadBanFile
pub fn load_ban_file(path: &Path, bans: &BanList) -> usize {
    if !path.is_file() {
        warn!(
            "IP bans configuration file ({}) does not exist or is not a regular file. No IP bans have been loaded.",
            path.display()
        );
        return bans.len();
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            for (index, raw) in text.lines().enumerate() {
                let line_number = index + 1;
                match parse_ban_line(raw) {
                    BanLine::Ignore => {}
                    BanLine::Entry { address, duration_ms } => bans.add(address, duration_ms),
                    BanLine::Invalid(reason) => warn!(
                        "Skipped IP ban entry ({reason}) in file ({}) at line {line_number}.",
                        path.display()
                    ),
                }
            }
        }
        Err(error) => warn!(
            "Error while reading IP bans configuration file ({}). Details: {error}",
            path.display()
        ),
    }
    info!("Loaded {} IP Bans.", bans.len());
    bans.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    #[test]
    fn parse_skips_blank_lines_and_comments() {
        assert_eq!(parse_ban_line(""), BanLine::Ignore);
        assert_eq!(parse_ban_line("   \t "), BanLine::Ignore);
        assert_eq!(parse_ban_line("# whole line comment"), BanLine::Ignore);
        assert_eq!(parse_ban_line("   # indented comment"), BanLine::Ignore);
    }

    #[test]
    fn parse_address_only_is_permanent() {
        assert_eq!(
            parse_ban_line("10.255.255.255"),
            BanLine::Entry {
                address: "10.255.255.255",
                duration_ms: 0
            }
        );
    }

    #[test]
    fn parse_duration_and_trailing_comment() {
        assert_eq!(
            parse_ban_line("  192.168.0.0 1237618800000  # bans the subnet"),
            BanLine::Entry {
                address: "192.168.0.0",
                duration_ms: 1_237_618_800_000
            }
        );
        assert_eq!(
            parse_ban_line("1.2.3.4#no space before the comment"),
            BanLine::Entry {
                address: "1.2.3.4",
                duration_ms: 0
            }
        );
    }

    #[test]
    fn parse_extra_tokens_are_ignored_and_negative_duration_kept() {
        assert_eq!(
            parse_ban_line("1.2.3.4 500 whatever"),
            BanLine::Entry {
                address: "1.2.3.4",
                duration_ms: 500
            }
        );
        assert_eq!(
            parse_ban_line("1.2.3.4 -5"),
            BanLine::Entry {
                address: "1.2.3.4",
                duration_ms: -5
            }
        );
    }

    #[test]
    fn parse_bad_duration_is_invalid() {
        assert!(matches!(parse_ban_line("1.2.3.4 soon"), BanLine::Invalid(_)));
        assert!(matches!(parse_ban_line("1.2.3.4 1.5"), BanLine::Invalid(_)));
    }

    #[test]
    fn exact_ban_and_permanent() {
        let bans = BanList::new();
        bans.add_at("10.0.0.1", 0, NOW);
        assert_eq!(bans.expiry("10.0.0.1"), Some(PERMANENT_BAN));
        assert!(bans.is_banned_at("10.0.0.1", NOW + 10_i64.pow(12)));
        assert!(!bans.is_banned_at("10.0.0.2", NOW));
    }

    #[test]
    fn wildcard_lookups() {
        let bans = BanList::new();
        bans.add_at("10.1.2.0", 0, NOW);
        assert!(bans.is_banned_at("10.1.2.77", NOW));
        assert!(!bans.is_banned_at("10.1.3.77", NOW));

        let bans = BanList::new();
        bans.add_at("10.1.0.0", 0, NOW);
        assert!(bans.is_banned_at("10.1.200.200", NOW));
        assert!(!bans.is_banned_at("10.2.0.1", NOW));

        let bans = BanList::new();
        bans.add_at("10.0.0.0", 0, NOW);
        assert!(bans.is_banned_at("10.9.9.9", NOW));
        assert!(!bans.is_banned_at("11.0.0.1", NOW));
    }

    #[test]
    fn timed_ban_expires_and_exact_entry_is_removed() {
        let bans = BanList::new();
        bans.add_at("10.0.0.1", 1000, NOW);
        assert_eq!(bans.expiry("10.0.0.1"), Some(NOW + 1000));
        assert!(bans.is_banned_at("10.0.0.1", NOW + 1000));
        assert!(!bans.is_banned_at("10.0.0.1", NOW + 1001));
        assert!(bans.expiry("10.0.0.1").is_none(), "expired exact ban is removed");
        assert!(!bans.is_banned_at("10.0.0.1", NOW));
    }

    #[test]
    fn expired_subnet_ban_reports_false_but_stays_in_the_list() {
        let bans = BanList::new();
        bans.add_at("10.1.0.0", 1000, NOW);
        assert!(!bans.is_banned_at("10.1.5.5", NOW + 5000));
        assert_eq!(
            bans.len(),
            1,
            "Java removes the queried address, not the subnet key"
        );
        assert!(!bans.is_banned_at("10.1.6.6", NOW + 5000));
    }

    #[test]
    fn exact_entry_wins_over_subnet() {
        let bans = BanList::new();
        bans.add_at("10.1.2.0", 0, NOW);
        bans.add_at("10.1.2.3", 100, NOW);
        // The exact entry decides, even though it expired and the subnet ban is permanent.
        assert!(!bans.is_banned_at("10.1.2.3", NOW + 500));
        assert!(bans.is_banned_at("10.1.2.4", NOW + 500));
    }

    #[test]
    fn add_keeps_the_first_entry() {
        let bans = BanList::new();
        bans.add_at("10.0.0.1", 1000, NOW);
        bans.add_at("10.0.0.1", 0, NOW);
        assert_eq!(bans.expiry("10.0.0.1"), Some(NOW + 1000));
        assert_eq!(bans.len(), 1);
    }

    #[test]
    fn overflowing_duration_acts_as_permanent() {
        let bans = BanList::new();
        bans.add_at("10.0.0.1", i64::MAX, NOW);
        assert!(bans.expiry("10.0.0.1").is_some_and(|expiry| expiry < 0));
        assert!(bans.is_banned_at("10.0.0.1", NOW + 10_i64.pow(12)));
    }

    #[test]
    fn short_addresses_do_not_panic() {
        let bans = BanList::new();
        bans.add_at("::1", 0, NOW);
        assert!(bans.is_banned_at("::1", NOW), "exact match still works");
        assert!(!bans.is_banned_at("fe80::2", NOW));
        assert!(!bans.is_banned_at("", NOW));
        assert!(!bans.is_banned_at("1.2", NOW));
    }

    #[test]
    fn trailing_dot_counts_like_java_split() {
        // "10.1.2." splits into three parts, so the `10.1.2.0` lookup works.
        let bans = BanList::new();
        bans.add_at("10.1.2.0", 0, NOW);
        assert!(bans.is_banned_at("10.1.2.", NOW));
    }

    #[test]
    fn remove_reports_whether_a_ban_existed() {
        let bans = BanList::new();
        bans.add_at("10.0.0.1", 0, NOW);
        assert!(bans.remove("10.0.0.1"));
        assert!(!bans.remove("10.0.0.1"));
        assert!(bans.is_empty());
    }

    #[test]
    fn load_file_registers_valid_lines_and_skips_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("banned_ip.cfg");
        std::fs::write(
            &path,
            "# comment\n\n10.255.255.255\r\n192.168.0.0 3600000   # an hour\nbroken.entry notanumber\n  172.16.0.1\t0\n",
        )
        .unwrap();
        let bans = BanList::new();
        assert_eq!(load_ban_file(&path, &bans), 3);
        assert_eq!(bans.expiry("10.255.255.255"), Some(PERMANENT_BAN));
        assert_eq!(bans.expiry("172.16.0.1"), Some(PERMANENT_BAN));
        assert!(
            bans.expiry("192.168.0.0")
                .is_some_and(|expiry| expiry != PERMANENT_BAN)
        );
        assert!(bans.expiry("broken.entry").is_none());
    }

    #[test]
    fn missing_file_loads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let bans = BanList::new();
        assert_eq!(load_ban_file(&dir.path().join("nope.cfg"), &bans), 0);
        // A directory is "not a regular file" too.
        assert_eq!(load_ban_file(dir.path(), &bans), 0);
    }
}
