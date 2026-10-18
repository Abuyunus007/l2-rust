//! Operations on the registered game server table: registering, going down, finding links.
// Java: org/l2jmobius/loginserver/data/GameServerTable.java (`register`,
//       `registerWithFirstAvailableId`, `GameServerInfo`)
// Java: org/l2jmobius/loginserver/GameServerThread.java (`attachGameServerInfo`, `setGameHosts`)
// Java: org/l2jmobius/loginserver/controller/LoginController.java (`isAccountInAnyGameServer`,
//       `getAccountOnGameServer`, `getCharactersOnAccount`)
//
// The Java `GameServerInfo` is one shared mutable object that a table, a thread and the client
// handlers all hold. Here the table owns the rows; everything that changes a row goes through
// a method on `GameServerTable` that locks the table for that one step. Rows handed out are
// copies, and the lock is never held across an `.await`.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use tracing::{info, warn};

use crate::gs::link::GsLink;
use crate::gs::subnet::{GameServerAddress, Subnet};
use crate::state::{GameServerInfo, GameServerTable, ServerStatus, lock};

impl GameServerInfo {
    /// Sets the status the game server reported, unless the login server itself is DOWN or
    /// GM-only, which then wins.
    // Java: GameServerInfo.setStatus
    pub fn set_status(&mut self, status: ServerStatus, login_status: ServerStatus) {
        self.status = match login_status {
            ServerStatus::Down => ServerStatus::Down,
            ServerStatus::GmOnly => ServerStatus::GmOnly,
            _ => status,
        };
    }

    /// The connection ended: not authed, no port, no link, status DOWN. The row stays in the
    /// table.
    // Java: GameServerInfo.setDown
    pub fn set_down(&mut self, login_status: ServerStatus) {
        self.authed = false;
        self.port = 0;
        self.link = None;
        // NOTE(java-quirk): `setStatus(STATUS_DOWN)` goes through the login server status check,
        // so while the login server is GM-only a game server that went down shows as GM-only.
        self.set_status(ServerStatus::Down, login_status);
    }

    /// Players on the game server (0 without a link).
    // Java: GameServerInfo.getCurrentPlayerCount
    pub fn current_player_count(&self) -> usize {
        self.link.as_ref().map_or(0, |link| link.player_count())
    }

    /// The host a client at `address` should connect to: the first entry whose subnet holds it.
    // Java: GameServerInfo.getServerAddress
    pub fn server_address(&self, address: IpAddr) -> Option<&str> {
        self.addresses
            .iter()
            .find(|entry| entry.subnet.contains(address))
            .map(|entry| entry.host.as_str())
    }

    /// The host stored in the `gameservers` table: the entry for `0.0.0.0`.
    // Java: GameServerInfo.getExternalHost
    pub fn external_host(&self) -> Option<String> {
        self.server_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .map(str::to_owned)
    }

    /// Replaces the address list with the pairs the game server sent. A pair whose subnet
    /// cannot be used is skipped with a warning.
    // Java: GameServerThread.setGameHosts
    pub(crate) fn set_game_hosts(&mut self, name: Option<&str>, subnets: &[String], hosts: &[String]) {
        info!(
            "Updated Gameserver [{}] {} IP's:",
            self.id,
            name.unwrap_or("null")
        );
        self.addresses.clear();
        for (subnet, host) in subnets.iter().zip(hosts) {
            match Subnet::parse(subnet) {
                Ok(parsed) => self.addresses.push(GameServerAddress {
                    subnet: parsed,
                    host: host.clone(),
                }),
                Err(error) => warn!("Failed to resolve hostname \"{subnet}\" - {error}"),
            }
        }
        for address in &self.addresses {
            info!("{}", address.describe());
        }
    }

    /// Ties the row to a connection and marks it authed.
    // Java: GameServerThread.attachGameServerInfo
    pub(crate) fn attach(
        &mut self,
        name: Option<&str>,
        link: Arc<GsLink>,
        port: u16,
        subnets: &[String],
        hosts: &[String],
        max_players: i32,
    ) {
        self.link = Some(link);
        self.port = port;
        self.set_game_hosts(name, subnets, hosts);
        self.max_players = max_players;
        self.authed = true;
    }
}

/// Adds `info` under `id` unless the id is taken. Gives the row back when it is.
// Java: GameServerTable.register
pub(crate) fn insert_if_absent(
    registered: &mut BTreeMap<i32, GameServerInfo>,
    id: i32,
    info: GameServerInfo,
) -> Result<(), GameServerInfo> {
    if registered.contains_key(&id) {
        return Err(info);
    }
    registered.insert(id, info);
    Ok(())
}

/// Adds `info` under the lowest id from `servername.xml` that has no registered server, and
/// returns that id. Gives the row back when every named id is taken.
// Java: GameServerTable.registerWithFirstAvailableId
pub(crate) fn insert_with_first_available_id(
    registered: &mut BTreeMap<i32, GameServerInfo>,
    names: &BTreeMap<i32, String>,
    mut info: GameServerInfo,
) -> Result<i32, GameServerInfo> {
    // NOTE(java-quirk): the candidates are the ids in `servername.xml`, not "any unused number".
    // Without a name file there is never a free id.
    let Some(id) = names.keys().copied().find(|id| !registered.contains_key(id)) else {
        return Err(info);
    };
    info.id = id;
    registered.insert(id, info);
    Ok(id)
}

impl GameServerTable {
    /// Registers `info` under `id`. False when a server is registered there already.
    // Java: GameServerTable.register
    pub fn register(&self, id: i32, info: GameServerInfo) -> bool {
        insert_if_absent(&mut lock(&self.registered), id, info).is_ok()
    }

    /// Registers `info` under the first free id of `servername.xml` and returns it.
    // Java: GameServerTable.registerWithFirstAvailableId
    pub fn register_with_first_available_id(&self, info: GameServerInfo) -> Option<i32> {
        insert_with_first_available_id(&mut lock(&self.registered), self.server_names(), info).ok()
    }

    /// Marks server `id` as down after its link ended. Only acts when `link_id` is the link the
    /// row currently has, so a late cleanup never touches a newer connection. True when it did.
    // Java: GameServerThread.run (finally block: `_gameServerInfo.setDown()`)
    pub fn set_down(&self, id: i32, link_id: u64, login_status: ServerStatus) -> bool {
        let mut registered = lock(&self.registered);
        match registered.get_mut(&id) {
            Some(info) if info.link.as_ref().is_some_and(|link| link.id() == link_id) => {
                info.set_down(login_status);
                true
            }
            _ => false,
        }
    }

    /// Changes row `id` in place and returns what `change` returns; `None` for an unknown id.
    /// `change` must not block or call back into the table.
    pub fn update<R>(&self, id: i32, change: impl FnOnce(&mut GameServerInfo) -> R) -> Option<R> {
        lock(&self.registered).get_mut(&id).map(change)
    }

    /// The link of server `id`, when it is connected.
    pub fn link(&self, id: i32) -> Option<Arc<GsLink>> {
        lock(&self.registered).get(&id).and_then(|info| info.link.clone())
    }

    /// The links of all connected servers, by server id.
    fn links(&self) -> Vec<(i32, Arc<GsLink>)> {
        lock(&self.registered)
            .iter()
            .filter_map(|(id, info)| info.link.clone().map(|link| (*id, link)))
            .collect()
    }

    /// Whether any connected game server has `account` online.
    // Java: LoginController.isAccountInAnyGameServer
    pub fn is_account_on_any_gs(&self, account: &str) -> bool {
        self.account_on_game_server(account).is_some()
    }

    /// The id of the game server that has `account` online.
    // Java: LoginController.getAccountOnGameServer
    pub fn account_on_game_server(&self, account: &str) -> Option<i32> {
        self.links()
            .into_iter()
            .find(|(_, link)| link.has_account(account))
            .map(|(id, _)| id)
    }

    /// Tells the game server that has `account` online to kick it. True when there was one (the
    /// caller then answers the new login with `ACCOUNT_IN_USE`).
    // Java: RequestAuthLogin (case ALREADY_ON_GS)
    pub fn kick_player(&self, account: &str) -> bool {
        let found = self
            .links()
            .into_iter()
            .find(|(_, link)| link.has_account(account));
        match found {
            Some((_, link)) => {
                link.kick_player(account);
                true
            }
            None => false,
        }
    }

    /// Asks every connected game server how many characters `account` has there.
    // Java: LoginController.getCharactersOnAccount
    /// Returns the ids of the servers that were asked, so the server list knows which
    /// `ReplyCharacters` to wait for.
    pub fn request_characters(&self, account: &str) -> Vec<i32> {
        let links = self.links();
        for (_, link) in &links {
            link.request_characters(account);
        }
        links.into_iter().map(|(id, _)| id).collect()
    }
}
