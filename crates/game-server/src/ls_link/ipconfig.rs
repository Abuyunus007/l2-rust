//! The subnet and host lists of `AuthRequest`: which address the login server shows to which
//! client network.
// Java: org/l2jmobius/gameserver/config/ServerConfig.java (`IPConfigData`)
// Java: dist/game/config/default-ipconfig.xml
//
// The login server answers a client with the host whose subnet contains the client's address,
// first match wins (`GameServerInfo.getServerAddress`), so the order of the pairs matters.

use std::borrow::Cow;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// Name inside the config directory. Java uses `./config/ipconfig.xml`; the shipped
/// `default-ipconfig.xml` is a template that is not read.
pub const IPCONFIG_FILE: &str = "ipconfig.xml";

/// Java's fallback for an empty list.
const ANY_SUBNET: &str = "0.0.0.0/0";
const LOCALHOST: &str = "127.0.0.1";

/// The parallel lists `GAME_SERVER_SUBNETS` and `GAME_SERVER_HOSTS`. `subnets[i]` belongs to
/// `hosts[i]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpConfig {
    pub subnets: Vec<String>,
    pub hosts: Vec<String>,
}

impl IpConfig {
    /// Reads `ipconfig.xml` from `config_dir`; without the file Java configures itself from
    /// the network interfaces ([`IpConfig::automatic`]).
    // Java: IPConfigData.load
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(IPCONFIG_FILE);
        if !path.is_file() {
            info!("Network Config: ipconfig.xml does not exist, using automatic configuration...");
            return Self::automatic();
        }
        info!("Network Config: ipconfig.xml exists, using manual configuration...");
        match std::fs::read(&path) {
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                // Java: `IXmlReader.parseFile` logs the failure and leaves the lists empty.
                warn!("Could not read file {}: {e}", path.display());
                Self::from_lists(Vec::new(), Vec::new())
            }
        }
    }

    /// The manual configuration: one `<define subnet=".." address=".."/>` per network, then
    /// the `address` of `<gameserver>` for everybody else (`0.0.0.0/0`).
    // Java: IPConfigData.parseDocument
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
                    // Java: the parse exception is logged and `parseDocument` never runs.
                    warn!("Could not parse {IPCONFIG_FILE}: {e}");
                    lists = Lists::default();
                    break;
                }
            }
        }
        Self::from_lists(lists.subnets, lists.hosts)
    }

    /// What Java builds from the network interfaces and `checkip.amazonaws.com` when there is
    /// no `ipconfig.xml`.
    ///
    /// NOTE: not ported. Std cannot list interfaces and an HTTP call at startup is not wanted,
    /// so this gives the result Java reaches on a machine with only a loopback interface and no
    /// internet: the loopback network gets 127.0.0.1, and so does everybody else. Real setups
    /// must provide `ipconfig.xml` (see `default-ipconfig.xml`).
    pub fn automatic() -> Self {
        warn!(
            "Network Config: automatic configuration is not supported by the Rust game server; \
             only 127.0.0.1 is announced. Create {IPCONFIG_FILE} (see default-ipconfig.xml) to \
             serve other networks."
        );
        Self {
            subnets: vec!["127.0.0.0/8".to_owned(), ANY_SUBNET.to_owned()],
            hosts: vec![LOCALHOST.to_owned(), LOCALHOST.to_owned()],
        }
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
        assert_eq!(IpConfig::load(dir.path()), IpConfig::automatic());
        std::fs::write(dir.path().join(IPCONFIG_FILE), DEFAULT).unwrap();
        assert_eq!(IpConfig::load(dir.path()), IpConfig::parse(DEFAULT));
    }

    #[test]
    fn lists_stay_parallel() {
        for config in [
            IpConfig::parse(DEFAULT),
            IpConfig::automatic(),
            IpConfig::parse(""),
        ] {
            assert_eq!(config.subnets.len(), config.hosts.len());
        }
    }
}
