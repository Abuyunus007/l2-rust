//! Subnets of a game server's address list: which address a client gets to see.
// Java: org/l2jmobius/loginserver/util/Subnet.java
// Java: org/l2jmobius/loginserver/data/GameServerTable.java (`GameServerInfo.GameServerAddress`)

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Why a subnet text could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubnetError {
    // NOTE(java-quirk): Java accepts a host name here (`InetAddress.getByName` looks it up in
    // DNS). The Rust version only takes IP literals: a login server should not block on DNS,
    // and subnets are IP literals in every shipped `ipconfig.xml`.
    #[error("{0:?} is not an IP address")]
    NotAnAddress(String),
    #[error("invalid prefix length {0:?}")]
    BadPrefix(String),
    #[error("prefix length {prefix} is longer than the address allows ({max})")]
    PrefixTooLong { prefix: u32, max: u32 },
}

/// An address with a prefix length, `a.b.c.d/n`. The stored address has the host bits cleared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subnet {
    address: Vec<u8>,
    mask: Vec<u8>,
}

/// The raw bytes Java's `InetAddress.getAddress()` gives. Java turns an IPv4-mapped IPv6
/// address (`::ffff:1.2.3.4`) into an `Inet4Address` when it parses one, so that is done here.
fn address_bytes(address: IpAddr) -> Vec<u8> {
    match address {
        IpAddr::V4(v4) => v4.octets().to_vec(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.octets().to_vec(),
            None => v6.octets().to_vec(),
        },
    }
}

impl Subnet {
    /// Parses `address[/prefix]`. Without a prefix the whole address counts (32 or 128 bits).
    // Java: Subnet(String)
    pub fn parse(input: &str) -> Result<Self, SubnetError> {
        let mut parts = input.split('/');
        // `split` always yields a first part.
        let host = parts.next().unwrap_or_default();
        let ip: IpAddr = host
            .parse()
            .map_err(|_| SubnetError::NotAnAddress(host.to_owned()))?;
        let mut address = address_bytes(ip);
        let max_prefix = u32::try_from(address.len() * 8).unwrap_or(u32::MAX);
        let prefix = match parts.next() {
            // Java: `Integer.parseInt(parts[1])`, which also accepts a sign.
            Some(text) => text
                .parse::<i32>()
                .map_err(|_| SubnetError::BadPrefix(text.to_owned()))?,
            None => i32::try_from(max_prefix).unwrap_or(i32::MAX),
        };
        let prefix = u32::try_from(prefix).map_err(|_| SubnetError::BadPrefix(prefix.to_string()))?;
        if prefix > max_prefix {
            return Err(SubnetError::PrefixTooLong {
                prefix,
                max: max_prefix,
            });
        }

        let full_bytes = (prefix / 8) as usize;
        let remaining_bits = prefix % 8;
        let mut mask = vec![0u8; address.len()];
        mask[..full_bytes].fill(0xFF);
        if remaining_bits > 0 {
            mask[full_bytes] = 0xFFu8 << (8 - remaining_bits);
        }
        for (byte, mask) in address.iter_mut().zip(&mask) {
            *byte &= mask;
        }
        Ok(Self { address, mask })
    }

    /// True when `candidate` lies in this subnet.
    // Java: Subnet.isInSubnet
    pub fn contains(&self, candidate: IpAddr) -> bool {
        // Java compares the byte arrays that `getAddress()` returns. A mapped IPv6 candidate is
        // already an IPv4 address there, which `address_bytes` reproduces.
        let candidate = address_bytes(candidate);
        if candidate.len() != self.address.len() {
            return false;
        }
        candidate
            .iter()
            .zip(&self.mask)
            .zip(&self.address)
            .all(|((byte, mask), network)| byte & mask == *network)
    }

    fn prefix_len(&self) -> u32 {
        self.mask.iter().map(|b| b.count_ones()).sum()
    }
}

impl fmt::Display for Subnet {
    // Java: Subnet.toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.address.as_slice() {
            &[a, b, c, d] => write!(f, "{}/{}", Ipv4Addr::new(a, b, c, d), self.prefix_len()),
            bytes => match <[u8; 16]>::try_from(bytes) {
                Ok(octets) => write!(f, "{}/{}", Ipv6Addr::from(octets), self.prefix_len()),
                Err(_) => write!(f, "Invalid address"),
            },
        }
    }
}

/// One entry of the address list a game server sends: clients inside `subnet` connect to `host`.
// Java: GameServerTable.GameServerInfo.GameServerAddress
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameServerAddress {
    pub subnet: Subnet,
    pub host: String,
}

impl GameServerAddress {
    /// `subnet` in CIDR form (or a bare address), `host` as sent by the game server.
    pub fn new(subnet: &str, host: &str) -> Result<Self, SubnetError> {
        Ok(Self {
            subnet: Subnet::parse(subnet)?,
            host: host.to_owned(),
        })
    }

    /// Java `GameServerAddress.toString`: `host/subnet`.
    pub fn describe(&self) -> String {
        format!("{}/{}", self.host, self.subnet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn host_bits_are_cleared_and_matching_uses_the_mask() {
        let subnet = Subnet::parse("192.168.1.77/24").unwrap();
        assert_eq!(subnet.to_string(), "192.168.1.0/24");
        assert!(subnet.contains(v4(192, 168, 1, 1)));
        assert!(subnet.contains(v4(192, 168, 1, 255)));
        assert!(!subnet.contains(v4(192, 168, 2, 1)));
    }

    #[test]
    fn partial_byte_prefix() {
        let subnet = Subnet::parse("10.0.0.0/12").unwrap();
        assert!(subnet.contains(v4(10, 15, 255, 255)));
        assert!(!subnet.contains(v4(10, 16, 0, 0)));
    }

    #[test]
    fn no_prefix_means_one_address() {
        let subnet = Subnet::parse("127.0.0.1").unwrap();
        assert_eq!(subnet.to_string(), "127.0.0.1/32");
        assert!(subnet.contains(v4(127, 0, 0, 1)));
        assert!(!subnet.contains(v4(127, 0, 0, 2)));
    }

    #[test]
    fn the_catch_all_subnet_contains_the_unspecified_address() {
        // `GameServerInfo.getExternalHost` asks for 0.0.0.0, which only a /0 style entry matches.
        let any = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        assert!(Subnet::parse("0.0.0.0/0").unwrap().contains(any));
        assert!(!Subnet::parse("10.0.0.0/8").unwrap().contains(any));
    }

    #[test]
    fn ipv4_and_ipv6_do_not_mix_except_for_mapped_addresses() {
        let subnet = Subnet::parse("192.168.0.0/16").unwrap();
        assert!(subnet.contains(IpAddr::V6(Ipv4Addr::new(192, 168, 3, 4).to_ipv6_mapped())));
        assert!(!subnet.contains(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        let v6 = Subnet::parse("fe80::/10").unwrap();
        assert!(!v6.contains(v4(10, 0, 0, 1)));
        assert!(v6.contains("fe80::1".parse().unwrap()));
        assert_eq!(v6.to_string(), "fe80::/10");
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(matches!(
            Subnet::parse("not-an-ip"),
            Err(SubnetError::NotAnAddress(_))
        ));
        assert!(matches!(
            Subnet::parse("10.0.0.0/x"),
            Err(SubnetError::BadPrefix(_))
        ));
        assert!(matches!(
            Subnet::parse("10.0.0.0/-1"),
            Err(SubnetError::BadPrefix(_))
        ));
        assert_eq!(
            Subnet::parse("10.0.0.0/33"),
            Err(SubnetError::PrefixTooLong { prefix: 33, max: 32 })
        );
    }

    #[test]
    fn address_text_has_host_then_subnet() {
        let address = GameServerAddress {
            subnet: Subnet::parse("10.0.0.0/8").unwrap(),
            host: "gs.example".to_owned(),
        };
        assert_eq!(address.describe(), "gs.example/10.0.0.0/8");
    }
}
