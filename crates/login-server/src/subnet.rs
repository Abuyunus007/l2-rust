//! Subnet matching for the address a game server shows to a client.
//!
//! Java: `loginserver/util/Subnet.java`, and `GameServerTable.GameServerInfo.GameServerAddress`
//! (a `Subnet` plus the address to send to clients inside it).
//!
//! Java's `Subnet` only knows the CIDR form (`192.168.0.0/16`, or a bare address for a single
//! host). There is no "255.255.255.0" mask form, so there is none here.
//!
//! Addresses are literals only. Java's `InetAddress.getByName` would also resolve host names
//! (and accept short forms like `127.1`); a game server with such a subnet is refused here, and
//! the link code reports it like Java's "Failed to resolve hostname" warning.

use std::fmt;
use std::net::IpAddr;

/// Why a subnet string was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubnetError {
    /// Java: `UnknownHostException` (here: not an IP literal).
    #[error("unknown host '{0}'")]
    UnknownHost(String),
    /// Java: `NumberFormatException` from `Integer.parseInt` on the prefix.
    #[error("invalid prefix '{0}'")]
    BadPrefix(String),
    /// Java: `IllegalArgumentException("Invalid prefix length: ...")`.
    #[error("Invalid prefix length: {0}")]
    PrefixOutOfRange(i32),
}

/// An address block: the network address (already masked) and the mask, 4 or 16 bytes.
// Java: Subnet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subnet {
    address: Vec<u8>,
    mask: Vec<u8>,
}

/// The bytes Java's `InetAddress.getAddress()` gives: 4 for IPv4 (also for an IPv4-mapped IPv6
/// address, which `getByName` turns into an `Inet4Address`), 16 for other IPv6.
fn address_bytes(ip: IpAddr) -> Vec<u8> {
    match ip.to_canonical() {
        IpAddr::V4(v4) => v4.octets().to_vec(),
        IpAddr::V6(v6) => v6.octets().to_vec(),
    }
}

/// `InetAddress.getByName(host)` for literals. NOTE(java-quirk): an empty string gives the
/// loopback address.
fn parse_host(host: &str) -> Option<Vec<u8>> {
    if host.is_empty() {
        return Some(vec![127, 0, 0, 1]);
    }
    let literal = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    literal.parse::<IpAddr>().ok().map(address_bytes)
}

impl Subnet {
    // Java: Subnet(String)
    pub fn parse(input: &str) -> Result<Self, SubnetError> {
        // Java `String.split("/")` drops trailing empty strings; "" gives [""].
        let mut parts: Vec<&str> = input.split('/').collect();
        while parts.len() > 1 && parts.last() == Some(&"") {
            parts.pop();
        }
        if parts.len() == 1 && parts[0].is_empty() && !input.is_empty() {
            // "/" or "//": the Java array is empty and `parts[0]` throws.
            return Err(SubnetError::UnknownHost(input.to_owned()));
        }
        let mut address =
            parse_host(parts[0]).ok_or_else(|| SubnetError::UnknownHost(parts[0].to_owned()))?;
        let v4 = address.len() == 4;

        let mut prefix_length: i32 = if v4 { 32 } else { 128 };
        if let Some(text) = parts.get(1) {
            prefix_length = text
                .parse()
                .map_err(|_| SubnetError::BadPrefix((*text).to_owned()))?;
        }
        let max_bits = i32::try_from(address.len() * 8).unwrap_or(i32::MAX);
        if !(0..=max_bits).contains(&prefix_length) {
            return Err(SubnetError::PrefixOutOfRange(prefix_length));
        }

        let prefix = usize::try_from(prefix_length).unwrap_or(0);
        let (full_bytes, remaining_bits) = (prefix / 8, prefix % 8);
        let mut mask = vec![0u8; address.len()];
        mask[..full_bytes].fill(0xFF);
        if remaining_bits > 0 {
            // `(byte) (0xFF << (8 - remainingBits))`
            mask[full_bytes] = (0xFFu32 << (8 - remaining_bits)) as u8;
        }
        for (byte, m) in address.iter_mut().zip(&mask) {
            *byte &= m;
        }
        Ok(Self { address, mask })
    }

    /// Java `isInSubnet(byte[])`.
    pub fn is_in_subnet(&self, address: &[u8]) -> bool {
        let mut address = address;
        if address.len() != self.address.len() {
            // Only an IPv4 subnet can match a longer address, and only an IPv4-in-IPv6 one
            // (::ffff:a.b.c.d).
            let v4 = self.address.len() == 4;
            if !v4 || address.len() != 16 {
                return false;
            }
            const PREFIX: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF];
            if address[..12] != PREFIX {
                return false;
            }
            address = &address[12..];
        }
        address
            .iter()
            .zip(&self.mask)
            .zip(&self.address)
            .all(|((a, m), n)| a & m == *n)
    }

    /// Java `equals(InetAddress)`, the way `GameServerInfo.getServerAddress` uses it.
    pub fn contains(&self, ip: IpAddr) -> bool {
        self.is_in_subnet(&address_bytes(ip))
    }

    /// The masked network address.
    pub fn address(&self) -> &[u8] {
        &self.address
    }

    pub fn prefix_length(&self) -> u32 {
        self.mask.iter().map(|b| b.count_ones()).sum()
    }
}

/// Java `toString`: `getHostAddress() + "/" + prefix` (IPv6 uncompressed, as Java prints it).
impl fmt::Display for Subnet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [a, b, c, d] = self.address[..] {
            write!(f, "{a}.{b}.{c}.{d}")?;
        } else {
            for (i, pair) in self.address.chunks(2).enumerate() {
                if i > 0 {
                    f.write_str(":")?;
                }
                write!(f, "{:x}", u16::from_be_bytes([pair[0], pair[1]]))?;
            }
        }
        write!(f, "/{}", self.prefix_length())
    }
}

/// One entry of a game server's host list: clients inside `subnet` are told to connect to
/// `server_address`.
// Java: GameServerTable.GameServerInfo.GameServerAddress
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameServerAddress {
    subnet: Subnet,
    server_address: String,
}

impl GameServerAddress {
    pub fn new(subnet: &str, server_address: &str) -> Result<Self, SubnetError> {
        Ok(Self {
            subnet: Subnet::parse(subnet)?,
            server_address: server_address.to_owned(),
        })
    }

    pub fn subnet(&self) -> &Subnet {
        &self.subnet
    }

    pub fn server_address(&self) -> &str {
        &self.server_address
    }

    /// True when `client` is inside the subnet.
    pub fn matches(&self, client: IpAddr) -> bool {
        self.subnet.contains(client)
    }
}

impl fmt::Display for GameServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.server_address, self.subnet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn cidr_blocks_match_like_java() {
        let lan = Subnet::parse("192.168.0.0/16").unwrap();
        assert!(lan.contains(ip("192.168.5.7")));
        assert!(lan.contains(ip("192.168.255.255")));
        assert!(!lan.contains(ip("192.169.0.1")));
        assert!(!lan.contains(ip("10.0.0.1")));

        let everyone = Subnet::parse("0.0.0.0/0").unwrap();
        assert!(everyone.contains(ip("8.8.8.8")));
        assert!(everyone.contains(ip("127.0.0.1")));
        // An IPv4 block does not match plain IPv6 addresses.
        assert!(!everyone.contains(ip("2001:db8::1")));
    }

    #[test]
    fn prefixes_that_are_not_byte_aligned_use_the_partial_mask() {
        let block = Subnet::parse("172.16.0.0/12").unwrap();
        assert!(block.contains(ip("172.16.0.1")));
        assert!(block.contains(ip("172.31.255.255")));
        assert!(!block.contains(ip("172.32.0.1")));
        assert!(!block.contains(ip("172.15.255.255")));

        let odd = Subnet::parse("10.0.0.128/25").unwrap();
        assert!(odd.contains(ip("10.0.0.200")));
        assert!(!odd.contains(ip("10.0.0.127")));
        assert_eq!(odd.prefix_length(), 25);
    }

    #[test]
    fn a_bare_address_is_a_single_host() {
        let host = Subnet::parse("203.0.113.9").unwrap();
        assert_eq!(host.prefix_length(), 32);
        assert!(host.contains(ip("203.0.113.9")));
        assert!(!host.contains(ip("203.0.113.10")));
        // A trailing slash is dropped by Java's split.
        assert_eq!(Subnet::parse("203.0.113.9/").unwrap(), host);
    }

    #[test]
    fn host_bits_are_cleared_in_the_stored_address() {
        let subnet = Subnet::parse("10.1.2.3/12").unwrap();
        assert_eq!(subnet.address(), [10, 0, 0, 0]);
        assert_eq!(subnet.to_string(), "10.0.0.0/12");
    }

    #[test]
    fn bad_prefixes_are_refused() {
        assert_eq!(
            Subnet::parse("10.0.0.0/33"),
            Err(SubnetError::PrefixOutOfRange(33))
        );
        assert_eq!(
            Subnet::parse("10.0.0.0/-1"),
            Err(SubnetError::PrefixOutOfRange(-1))
        );
        assert_eq!(
            Subnet::parse("10.0.0.0/abc"),
            Err(SubnetError::BadPrefix("abc".into()))
        );
        assert!(Subnet::parse("10.0.0.0/32").is_ok());
        assert!(matches!(
            Subnet::parse("not-an-address/8"),
            Err(SubnetError::UnknownHost(_))
        ));
        assert!(matches!(Subnet::parse("/"), Err(SubnetError::UnknownHost(_))));
    }

    #[test]
    fn an_empty_host_is_the_loopback_address() {
        // NOTE(java-quirk): `InetAddress.getByName("")` is the loopback address.
        assert_eq!(Subnet::parse("").unwrap().address(), [127, 0, 0, 1]);
        assert_eq!(Subnet::parse("/8").unwrap().address(), [127, 0, 0, 0]);
    }

    #[test]
    fn ipv4_in_ipv6_addresses_match_ipv4_blocks() {
        let lan = Subnet::parse("192.168.0.0/16").unwrap();
        // Raw 16-byte form, like Java's length-mismatch branch.
        let mut mapped = [0u8; 16];
        mapped[10] = 0xFF;
        mapped[11] = 0xFF;
        mapped[12..].copy_from_slice(&[192, 168, 1, 1]);
        assert!(lan.is_in_subnet(&mapped));
        mapped[12..].copy_from_slice(&[10, 0, 0, 1]);
        assert!(!lan.is_in_subnet(&mapped));
        // Not the ::ffff: prefix.
        let mut other = [0u8; 16];
        other[0] = 0x20;
        other[12..].copy_from_slice(&[192, 168, 1, 1]);
        assert!(!lan.is_in_subnet(&other));
        // Wrong lengths.
        assert!(!lan.is_in_subnet(&[192, 168, 1]));
        // The same through IpAddr (Java's getByName makes it an Inet4Address).
        assert!(lan.contains(ip("::ffff:192.168.1.1")));
        assert!(!lan.contains(ip("::ffff:10.0.0.1")));
    }

    #[test]
    fn ipv6_blocks() {
        let link_local = Subnet::parse("fe80::/10").unwrap();
        assert!(link_local.contains(ip("fe80::1")));
        assert!(link_local.contains(ip("febf::1")));
        assert!(!link_local.contains(ip("fec0::1")));
        assert!(
            !link_local.contains(ip("192.168.0.1")),
            "IPv4 against an IPv6 block"
        );
        assert_eq!(link_local.to_string(), "fe80:0:0:0:0:0:0:0/10");
        assert_eq!(Subnet::parse("[::1]").unwrap().prefix_length(), 128);
    }

    #[test]
    fn game_server_address_pairs_a_subnet_with_the_host_to_send() {
        let entry = GameServerAddress::new("192.168.0.0/16", "192.168.0.5").unwrap();
        assert!(entry.matches(ip("192.168.7.7")));
        assert!(!entry.matches(ip("1.2.3.4")));
        assert_eq!(entry.server_address(), "192.168.0.5");
        assert_eq!(entry.to_string(), "192.168.0.5/192.168.0.0/16");
        assert!(GameServerAddress::new("garbage", "1.2.3.4").is_err());
    }
}
