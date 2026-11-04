//! `config/Custom/DualboxCheck.ini`: how many connections one IP address may have in the game,
//! in the Olympiad, in an event and in the offline play (Java `AntiFeedManager`).
// The counters are keyed by Java's `String.hashCode()` of the client's address, like Java, so two
// addresses with the same hash share a counter. The whitelist of the config is keyed by
// `InetAddress.hashCode()` (the address as a number), a different number for the same address.
//
// The other owners of an event id call the `anti_feed_*` methods: `OLYMPIAD_ID` is used by the
// Olympiad registration (`olympiad/`), `L2EVENT_ID` by the event scripts and `OFFLINE_PLAY` by the
// offline play.

use std::collections::BTreeMap;
use std::net::IpAddr;

use l2_util::ObjectId;

use crate::world::World;

/// `AntiFeedManager.GAME_ID`.
pub const GAME_ID: i32 = 0;
/// `AntiFeedManager.OLYMPIAD_ID`.
pub const OLYMPIAD_ID: i32 = 1;
/// `AntiFeedManager.TVT_ID`.
pub const TVT_ID: i32 = 2;
/// `AntiFeedManager.L2EVENT_ID`.
pub const L2EVENT_ID: i32 = 3;
/// `AntiFeedManager.OFFLINE_PLAY`.
pub const OFFLINE_PLAY: i32 = 4;

/// The page `CharacterSelect` shows to a client that is over the limit.
pub const IP_RESTRICTION_PAGE: &str = "data/html/mods/IPRestriction.htm";

/// Java `String.hashCode()`.
pub fn java_string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

/// `AntiFeedManager._eventIPs`: per event, the connections per address hash.
#[derive(Debug, Default)]
pub struct AntiFeed {
    events: BTreeMap<i32, BTreeMap<i32, i32>>,
}

impl World {
    /// `AntiFeedManager.registerEvent(eventId)`.
    pub fn anti_feed_register_event(&mut self, event_id: i32) {
        self.custom_misc.anti_feed.events.entry(event_id).or_default();
    }

    /// The address text Java's `client.getIp()` gives.
    fn client_ip_text(ip: Option<IpAddr>) -> Option<String> {
        ip.map(|ip| ip.to_string())
    }

    /// `AntiFeedManager.tryAddClient(eventId, client, max)`: counts the connection when the
    /// address has fewer than `max` plus its whitelist extra. `ip` is the client's address
    /// (`None`: no client, refused).
    pub fn anti_feed_try_add_client(&mut self, event_id: i32, ip: Option<IpAddr>, max: i32) -> bool {
        let Some(text) = Self::client_ip_text(ip) else {
            return false;
        };
        let hash = java_string_hash(&text);
        let extra = self
            .settings
            .custom_misc
            .dualbox
            .whitelist
            .get(&hash)
            .copied()
            .unwrap_or(0);
        let Some(event) = self.custom_misc.anti_feed.events.get_mut(&event_id) else {
            return false; // no such event registered
        };
        let count = event.entry(hash).or_insert(0);
        if (*count).wrapping_add(1) <= max.wrapping_add(extra) {
            *count += 1;
            return true;
        }
        false
    }

    /// `AntiFeedManager.tryAddPlayer(eventId, player, max)`.
    pub fn anti_feed_try_add_player(&mut self, event_id: i32, player: ObjectId, max: i32) -> bool {
        let ip = self.players.get(&player).and_then(|p| p.client_ip);
        self.anti_feed_try_add_client(event_id, ip, max)
    }

    /// `AntiFeedManager.removeClient(eventId, client)`: one connection less; `true` when the
    /// address still has connections (Java's `computeIfPresent` result).
    pub fn anti_feed_remove_client(&mut self, event_id: i32, ip: Option<IpAddr>) -> bool {
        let Some(text) = Self::client_ip_text(ip) else {
            return false;
        };
        let hash = java_string_hash(&text);
        let Some(event) = self.custom_misc.anti_feed.events.get_mut(&event_id) else {
            return false;
        };
        let Some(count) = event.get_mut(&hash) else {
            return false;
        };
        *count -= 1;
        if *count == 0 {
            event.remove(&hash);
            return false;
        }
        true
    }

    /// `AntiFeedManager.removePlayer(eventId, player)`.
    pub fn anti_feed_remove_player(&mut self, event_id: i32, player: ObjectId) -> bool {
        let ip = self.players.get(&player).and_then(|p| p.client_ip);
        self.anti_feed_remove_client(event_id, ip)
    }

    /// `AntiFeedManager.clear(eventId)`.
    pub fn anti_feed_clear(&mut self, event_id: i32) {
        if let Some(event) = self.custom_misc.anti_feed.events.get_mut(&event_id) {
            event.clear();
        }
    }

    /// `AntiFeedManager.getLimit(client, max)`: the limit with the whitelist.
    pub fn anti_feed_limit(&self, ip: Option<IpAddr>, max: i32) -> i32 {
        let Some(text) = Self::client_ip_text(ip) else {
            return max;
        };
        match self
            .settings
            .custom_misc
            .dualbox
            .whitelist
            .get(&java_string_hash(&text))
        {
            // "DualboxCheckWhitelist takes 0 or negative value for unlimited number of connections."
            Some(extra) if *extra < 1 => i32::MAX,
            Some(extra) => max.wrapping_add(*extra),
            None => max,
        }
    }

    /// `CharacterSelect`: the `GAME_ID` limit of one IP address for a character below access level
    /// 100. `Some(page)` is the page to show instead of selecting the character; `None` lets the
    /// client select (the connection is counted).
    pub fn dualbox_check_select(&mut self, ip: Option<IpAddr>, access_level: i32) -> Option<String> {
        let max = self.settings.custom_misc.dualbox.max_players_per_ip;
        if access_level >= 100 || max <= 0 {
            return None;
        }
        self.anti_feed_register_event(GAME_ID);
        if self.anti_feed_try_add_client(GAME_ID, ip, max) {
            return None;
        }
        let limit = self.anti_feed_limit(ip, max);
        Some(self.ip_restriction_page(limit))
    }

    /// `data/html/mods/IPRestriction.htm` with `%max%` filled in.
    pub(crate) fn ip_restriction_page(&self, limit: i32) -> String {
        let page = self
            .talk
            .html
            .as_ref()
            .and_then(|cache| cache.get_localized("", IP_RESTRICTION_PAGE))
            .map_or_else(
                || format!("<html><body>My Text is missing:<br>{IP_RESTRICTION_PAGE}</body></html>"),
                str::to_owned,
            );
        page.replace("%max%", &limit.to_string())
    }

    /// `Disconnection.onDisconnect` for a client that selected a character and never entered the
    /// world: the connection is not counted any more.
    pub fn dualbox_release_select(&mut self, ip: Option<IpAddr>) {
        self.anti_feed_on_disconnect(ip, false);
    }

    /// `AntiFeedManager.onDisconnect(client)` for a player that is not an offline trader. In the
    /// Olympiad event the count only goes down for a registered or fighting noble.
    fn anti_feed_on_disconnect(&mut self, ip: Option<IpAddr>, olympiad_participant: bool) {
        let Some(text) = Self::client_ip_text(ip) else {
            return;
        };
        let hash = java_string_hash(&text);
        let ids: Vec<i32> = self.custom_misc.anti_feed.events.keys().copied().collect();
        for event_id in ids {
            if event_id == OLYMPIAD_ID {
                if olympiad_participant
                    && let Some(count) = self
                        .custom_misc
                        .anti_feed
                        .events
                        .get_mut(&event_id)
                        .and_then(|e| e.get_mut(&hash))
                {
                    *count -= 1;
                }
            } else {
                self.anti_feed_remove_client(event_id, ip);
            }
        }
    }

    /// `OlympiadManager.isRegistered(player) || player.getOlympiadGameId() != -1`.
    fn olympiad_participant(&self, id: ObjectId) -> bool {
        self.olympiad_is_registered(id) || self.olympiad_game_id(id) != -1
    }

    /// `Disconnection`: a player leaves. An offline trader is skipped (`isInOfflineMode`).
    pub(crate) fn dualbox_on_leave(&mut self, id: ObjectId) {
        if self.custom_misc.anti_feed.events.is_empty() || self.offline.traders.contains_key(&id) {
            return;
        }
        let Some(ip) = self.players.get(&id).map(|p| p.client_ip) else {
            return;
        };
        let olympiad = self.olympiad_participant(id);
        self.anti_feed_on_disconnect(ip, olympiad);
    }

    /// `OfflineTraderTable.enteredOfflineMode`: unless offline traders count, the trader's
    /// connection is not counted any more.
    pub(crate) fn dualbox_on_offline_mode(&mut self, id: ObjectId) {
        if self.settings.custom_misc.dualbox.count_offline_traders {
            return;
        }
        let Some(ip) = self.players.get(&id).map(|p| p.client_ip) else {
            return;
        };
        let olympiad = self.olympiad_participant(id);
        self.anti_feed_on_disconnect(ip, olympiad);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_string_hash_is_javas() {
        assert_eq!(java_string_hash(""), 0);
        assert_eq!(java_string_hash("a"), 97);
        assert_eq!(java_string_hash("127.0.0.1"), 1_505_998_205);
        assert_eq!(java_string_hash("Hello"), 69_609_650);
    }
}
