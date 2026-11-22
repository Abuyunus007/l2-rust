//! The subnet and host lists of `AuthRequest`: which address the login server shows to which
//! client network.
// The login server answers a client with the host whose subnet contains the client's address,
// first match wins (`GameServerInfo.getServerAddress`), so the order of the pairs matters.

use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// Name inside the config directory. Java uses `./config/ipconfig.xml`; the shipped
/// `default-ipconfig.xml` is a template that is not read.
pub const IPCONFIG_FILE: &str = "ipconfig.xml";

/// Java's fallback for an empty list.
const ANY_SUBNET: &str = "0.0.0.0/0";
const LOCALHOST: &str = "127.0.0.1";

/// Where Java asks for the external address (`autoIpConfig`).
const CHECKIP_HOST: &str = "checkip.amazonaws.com";
/// Connect, read and write limit (the name lookup keeps the resolver's own limits). Java waits as
/// long as the socket does; a server without internet should not stall its startup for long.
const CHECKIP_TIMEOUT: Duration = Duration::from_secs(5);

/// The parallel lists `GAME_SERVER_SUBNETS` and `GAME_SERVER_HOSTS`. `subnets[i]` belongs to
/// `hosts[i]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpConfig {
    pub subnets: Vec<String>,
    pub hosts: Vec<String>,
}

impl IpConfig {
    /// Reads `ipconfig.xml` from `config_dir`; without the file the server configures itself
    /// from the network interfaces and the external address, as Java does
    /// ([`IpConfig::automatic`]).
    pub fn load(config_dir: &Path) -> Self {
        Self::read(config_dir).unwrap_or_else(Self::automatic)
    }

    /// The manual configuration of `config_dir`, `None` when there is no `ipconfig.xml`.
    fn read(config_dir: &Path) -> Option<Self> {
        let path = config_dir.join(IPCONFIG_FILE);
        if !path.is_file() {
            info!("Network Config: ipconfig.xml does not exist, using automatic configuration...");
            return None;
        }
        info!("Network Config: ipconfig.xml exists, using manual configuration...");
        Some(match std::fs::read(&path) {
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!("Could not read file {}: {e}", path.display());
                Self::from_lists(Vec::new(), Vec::new())
            }
        })
    }

    /// The manual configuration: one `<define subnet=".." address=".."/>` per network, then
    /// the `address` of `<gameserver>` for everybody else (`0.0.0.0/0`).
    pub fn parse(xml: &str) -> Self {
        let mut lists = Lists::default();
        let mut reader = Reader::from_str(xml);
        // The `address` of the <gameserver> element being read; `Some` while inside it.
        let mut open: Option<Option<String>> = None;
        loop {
            match reader.read_event() {
                Ok(Event::Start(element)) if is(&element, "gameserver") && open.is_none() => {
                    open = Some(attribute(&element, "address"));
                }
                Ok(Event::Empty(element)) if is(&element, "gameserver") && open.is_none() => {
                    lists.finish_gameserver(attribute(&element, "address"));
                }
                Ok(Event::Start(element) | Event::Empty(element))
                    if open.is_some() && is(&element, "define") =>
                {
                    lists.add_define(&element);
                }
                Ok(Event::End(element)) if open.is_some() && is_end(&element, "gameserver") => {
                    lists.finish_gameserver(open.take().flatten());
                }
                Ok(Event::Eof) => {
                    if open.is_some() {
                        // Java's DOM parser rejects the whole file; nothing is read.
                        warn!("Could not parse {IPCONFIG_FILE}: <gameserver> is not closed.");
                        lists = Lists::default();
                    }
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("Could not parse {IPCONFIG_FILE}: {e}");
                    lists = Lists::default();
                    break;
                }
            }
        }
        Self::from_lists(lists.subnets, lists.hosts)
    }

    /// Java `autoIpConfig`: every IPv4 network of the interfaces that are up gets this server's
    /// address in it, everybody else the external address from `checkip.amazonaws.com`
    /// (127.0.0.1 without internet).
    pub fn automatic() -> Self {
        Self::from_networks(interface_networks(), external_address())
    }

    /// The lists of [`IpConfig::automatic`]: one entry per network in interface order (the first
    /// address of a network wins), then the external address for `0.0.0.0/0`.
    fn from_networks(networks: impl IntoIterator<Item = (Ipv4Addr, u8)>, external: Ipv4Addr) -> Self {
        let (mut subnets, mut hosts) = (Vec::new(), Vec::new());
        for (address, prefix) in networks {
            let prefix = prefix.min(32);
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            let subnet = format!("{}/{prefix}", Ipv4Addr::from(u32::from(address) & mask));
            if subnet != ANY_SUBNET && !subnets.contains(&subnet) {
                info!("Network Config: Adding new subnet: {subnet} address: {address}");
                subnets.push(subnet);
                hosts.push(address.to_string());
            }
        }
        info!("Network Config: Adding new subnet: {ANY_SUBNET} address: {external}");
        subnets.push(ANY_SUBNET.to_owned());
        hosts.push(external.to_string());
        Self { subnets, hosts }
    }

    /// `getSubnets()` / `getHosts()`: an empty list becomes the catch-all entry.
    fn from_lists(subnets: Vec<String>, hosts: Vec<String>) -> Self {
        Self {
            subnets: if subnets.is_empty() {
                vec![ANY_SUBNET.to_owned()]
            } else {
                subnets
            },
            hosts: if hosts.is_empty() {
                vec![LOCALHOST.to_owned()]
            } else {
                hosts
            },
        }
    }
}

#[derive(Default)]
struct Lists {
    subnets: Vec<String>,
    hosts: Vec<String>,
}

impl Lists {
    fn add_define(&mut self, element: &BytesStart<'_>) {
        // Java throws a NullPointerException for a define without both attributes and loses the
        // rest of the file; skipping the define is the least surprising equivalent.
        match (attribute(element, "subnet"), attribute(element, "address")) {
            (Some(subnet), Some(address)) => {
                self.subnets.push(subnet);
                self.hosts.push(address);
            }
            _ => warn!("Ignoring a <define> without subnet and address in {IPCONFIG_FILE}."),
        }
    }

    /// The end of `<gameserver>`: its own address answers every network not listed above.
    fn finish_gameserver(&mut self, address: Option<String>) {
        let address = address.unwrap_or_else(|| {
            warn!("Failed to load {IPCONFIG_FILE} file - default server address is missing.");
            LOCALHOST.to_owned()
        });
        self.hosts.push(address);
        self.subnets.push(ANY_SUBNET.to_owned());
    }
}

/// The IPv4 networks of this machine's interfaces, as `(address, prefix length)`.
fn interface_networks() -> Vec<(Ipv4Addr, u8)> {
    let interfaces = match if_addrs::get_if_addrs() {
        Ok(interfaces) => interfaces,
        Err(e) => {
            // Java exits here; the server can still serve the external address.
            warn!("Network Config: Configuration failed please configure manually using ipconfig.xml: {e}");
            return Vec::new();
        }
    };
    // Java skips interfaces that are down and those without a 6-byte hardware address except
    // the loopback (tunnels, PPP); `is_p2p` is the closest the interface list tells.
    interfaces
        .iter()
        .filter_map(|interface| match &interface.addr {
            if_addrs::IfAddr::V4(v4) if interface.is_oper_up() && !interface.is_p2p() => {
                Some((v4.ip, v4.prefixlen))
            }
            _ => None,
        })
        .collect()
}

/// The external address as `checkip.amazonaws.com` sees this server; 127.0.0.1 when it cannot
/// be asked or answers something that is not an IPv4 address.
fn external_address() -> Ipv4Addr {
    ask_checkip().unwrap_or_else(|| {
        info!("Failed to connect to {CHECKIP_HOST} please check your internet connection using 127.0.0.1!");
        Ipv4Addr::LOCALHOST
    })
}

// shortcut: plain HTTP/1.0 over std (no HTTP client dependency), like Java's `URL.openStream`;
// the answer is validated, so a wrong one only costs the external entry.
fn ask_checkip() -> Option<Ipv4Addr> {
    let address = (CHECKIP_HOST, 80).to_socket_addrs().ok()?.find(|a| a.is_ipv4())?;
    let mut stream = TcpStream::connect_timeout(&address, CHECKIP_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(CHECKIP_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(CHECKIP_TIMEOUT)).ok()?;
    let request = format!("GET / HTTP/1.0\r\nHost: {CHECKIP_HOST}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = Vec::new();
    stream.take(4096).read_to_end(&mut response).ok()?;
    parse_checkip_response(&response)
}

/// The address in an HTTP answer of `checkip.amazonaws.com`: status 200, the first body line.
fn parse_checkip_response(response: &[u8]) -> Option<Ipv4Addr> {
    let text = std::str::from_utf8(response).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    if head.lines().next()?.split_whitespace().nth(1) != Some("200") {
        return None;
    }
    body.lines().next()?.trim().parse().ok()
}

fn is(element: &BytesStart<'_>, name: &str) -> bool {
    element.name().as_ref().eq_ignore_ascii_case(name)
}

fn is_end(element: &quick_xml::events::BytesEnd<'_>, name: &str) -> bool {
    element.name().as_ref().eq_ignore_ascii_case(name)
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(Cow::into_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Note: comment -->
<gameserver address="203.0.113.7" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../data/xsd/ipconfig.xsd">
	<!-- Localhost here -->
	<define subnet="127.0.0.0/8" address="127.0.0.1" />
	<define subnet="10.0.0.0/8" address="10.0.0.0" />
	<define subnet="192.168.0.0/16" address="192.168.0.0" />
</gameserver>"#;

    #[test]
    fn defines_come_first_then_the_default_for_everybody() {
        let config = IpConfig::parse(DEFAULT);
        assert_eq!(
            config.subnets,
            ["127.0.0.0/8", "10.0.0.0/8", "192.168.0.0/16", "0.0.0.0/0"]
        );
        assert_eq!(
            config.hosts,
            ["127.0.0.1", "10.0.0.0", "192.168.0.0", "203.0.113.7"]
        );
    }

    #[test]
    fn a_missing_default_address_is_localhost() {
        let config =
            IpConfig::parse(r#"<gameserver><define subnet="10.0.0.0/8" address="10.1.1.1"/></gameserver>"#);
        assert_eq!(config.subnets, ["10.0.0.0/8", "0.0.0.0/0"]);
        assert_eq!(config.hosts, ["10.1.1.1", "127.0.0.1"]);
    }

    #[test]
    fn an_empty_gameserver_element_still_has_the_default() {
        let config = IpConfig::parse(r#"<gameserver address="1.2.3.4"/>"#);
        assert_eq!(config.subnets, ["0.0.0.0/0"]);
        assert_eq!(config.hosts, ["1.2.3.4"]);
    }

    #[test]
    fn other_documents_give_java_s_fallback_lists() {
        for xml in [
            "",
            "<other/>",
            "<gameserver",
            "<gameserver address=\"1.1.1.1\">",
            "not xml at all",
        ] {
            let config = IpConfig::parse(xml);
            assert_eq!(config.subnets, ["0.0.0.0/0"], "{xml:?}");
            assert_eq!(config.hosts, ["127.0.0.1"], "{xml:?}");
        }
    }

    #[test]
    fn a_define_without_attributes_is_skipped() {
        let config = IpConfig::parse(
            r#"<gameserver address="1.2.3.4"><define subnet="10.0.0.0/8"/><define subnet="11.0.0.0/8" address="11.0.0.1"/></gameserver>"#,
        );
        assert_eq!(config.subnets, ["11.0.0.0/8", "0.0.0.0/0"]);
        assert_eq!(config.hosts, ["11.0.0.1", "1.2.3.4"]);
    }

    #[test]
    fn element_names_are_case_insensitive_like_java() {
        let config = IpConfig::parse(
            r#"<GameServer address="1.2.3.4"><Define subnet="10.0.0.0/8" address="10.0.0.1"/></GameServer>"#,
        );
        assert_eq!(config.hosts, ["10.0.0.1", "1.2.3.4"]);
    }

    #[test]
    fn load_reads_the_file_and_falls_back_without_it() {
        let dir = tempfile::tempdir().unwrap();
        // `None`: `load` configures itself (`automatic`, not called here: it asks the internet).
        assert_eq!(IpConfig::read(dir.path()), None);
        std::fs::write(dir.path().join(IPCONFIG_FILE), DEFAULT).unwrap();
        assert_eq!(IpConfig::load(dir.path()), IpConfig::parse(DEFAULT));
    }

    #[test]
    fn automatic_lists_one_entry_per_network_then_the_external_address() {
        let config = IpConfig::from_networks(
            [
                (Ipv4Addr::LOCALHOST, 8),
                (Ipv4Addr::new(192, 168, 178, 145), 24),
                // A second address of the same network: the first one stays.
                (Ipv4Addr::new(192, 168, 178, 146), 24),
                (Ipv4Addr::new(172, 18, 112, 1), 20),
                (Ipv4Addr::new(10, 1, 2, 3), 32),
                // A default-route prefix would shadow the external entry: Java skips it.
                (Ipv4Addr::new(100, 64, 0, 1), 0),
            ],
            Ipv4Addr::new(203, 0, 113, 7),
        );
        assert_eq!(
            config.subnets,
            [
                "127.0.0.0/8",
                "192.168.178.0/24",
                "172.18.112.0/20",
                "10.1.2.3/32",
                "0.0.0.0/0"
            ]
        );
        assert_eq!(
            config.hosts,
            [
                "127.0.0.1",
                "192.168.178.145",
                "172.18.112.1",
                "10.1.2.3",
                "203.0.113.7"
            ]
        );
    }

    #[test]
    fn no_interfaces_leave_only_the_external_address() {
        let config = IpConfig::from_networks([], Ipv4Addr::LOCALHOST);
        assert_eq!(config.subnets, ["0.0.0.0/0"]);
        assert_eq!(config.hosts, ["127.0.0.1"]);
    }

    #[test]
    fn checkip_answers_are_read_only_when_they_are_an_address() {
        let ok = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n203.0.113.7\n";
        assert_eq!(parse_checkip_response(ok), Some(Ipv4Addr::new(203, 0, 113, 7)));
        for bad in [
            &b"HTTP/1.1 503 Service Unavailable\r\n\r\n203.0.113.7\n"[..],
            b"HTTP/1.1 200 OK\r\n\r\n<html>captive portal</html>",
            b"HTTP/1.1 200 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\n",
            b"",
            b"\xff\xfe",
        ] {
            assert_eq!(parse_checkip_response(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn lists_stay_parallel() {
        for config in [
            IpConfig::parse(DEFAULT),
            IpConfig::from_networks([(Ipv4Addr::LOCALHOST, 8)], Ipv4Addr::LOCALHOST),
            IpConfig::parse(""),
        ] {
            assert_eq!(config.subnets.len(), config.hosts.len());
        }
    }
}
