//! The faction system (Good against Evil), `config/Custom/FactionSystem.ini` (M9-C04).
// Java: config/custom/FactionSystemConfig.java, entity/actor/Player.java (`_isGood`, `_isEvil`,
// Java: setGood, setEvil), entity/World.java (`_allGoodPlayers`, `_allEvilPlayers`),
// Java: data/xml/MapRegionData.java (respawn at the base), network/clientpackets/EnterWorld.java,
// Java: network/clientpackets/CharacterSelect.java (the balance check)
//
// A player's faction is `characters.faction` (0 none, 1 good, 2 evil), kept in
// `PlayerProfile::faction`. Java's two player lists are the players of the world with that value,
// so they are counted from the player map. The selection itself (the NPC, the html pages) is the
// script `custom/FactionSystem` in `l2-scripts`; the PvP, guard and chat rules are in the files of
// the systems they change, each behind `FactionSettings::enabled`.

use l2_protocol_game::server::ExShowScreenMessage;
use l2_util::ObjectId;

use crate::world::World;

/// `characters.faction` of a player on the good side.
pub const FACTION_GOOD: i32 = 1;
/// `characters.faction` of a player on the evil side.
pub const FACTION_EVIL: i32 = 2;

/// A place of the ini file: `x,y,z` and, for the manager, the heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FactionLocation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl FactionLocation {
    const fn new(x: i32, y: i32, z: i32, heading: i32) -> Self {
        Self { x, y, z, heading }
    }
}

/// `FactionSystemConfig`. `Default` is the Java default of each key (the system is off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactionSettings {
    /// `EnableFactionSystem`.
    pub enabled: bool,
    /// `StartingLocation` (used by the character creation).
    pub starting_location: FactionLocation,
    /// `ManagerSpawnLocation`.
    pub manager_location: FactionLocation,
    /// `GoodBaseLocation`.
    pub good_base_location: FactionLocation,
    /// `EvilBaseLocation`.
    pub evil_base_location: FactionLocation,
    /// `GoodTeamName`.
    pub good_team_name: String,
    /// `EvilTeamName`.
    pub evil_team_name: String,
    /// `GoodNameColor`, `Integer.decode("0x" + text)`.
    pub good_name_color: i32,
    /// `EvilNameColor`.
    pub evil_name_color: i32,
    /// `EnableFactionGuards`.
    pub guards_enabled: bool,
    /// `RespawnAtFactionBase`.
    pub respawn_at_base: bool,
    /// `FactionAutoNobless`.
    pub auto_nobless: bool,
    /// `EnableFactionChat` (`FACTION_SPECIFIC_CHAT`).
    pub specific_chat: bool,
    /// `BalanceOnlinePlayers`.
    pub balance_online_players: bool,
    /// `BalancePlayerExceedLimit`.
    pub balance_player_exceed_limit: i32,
}

impl Default for FactionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            starting_location: FactionLocation::new(85332, 16199, -1252, 0),
            manager_location: FactionLocation::new(85712, 15974, -1260, 26808),
            good_base_location: FactionLocation::new(45306, 48878, -3058, 0),
            evil_base_location: FactionLocation::new(-44037, -113283, -237, 0),
            good_team_name: "Good".to_owned(),
            evil_team_name: "Evil".to_owned(),
            good_name_color: 0x00FF00,
            evil_name_color: 0x0000FF,
            guards_enabled: true,
            respawn_at_base: true,
            auto_nobless: false,
            specific_chat: true,
            balance_online_players: true,
            balance_player_exceed_limit: 20,
        }
    }
}

impl World {
    /// `FactionSystemConfig.FACTION_SYSTEM_ENABLED`.
    pub fn faction_system_enabled(&self) -> bool {
        self.settings.faction.enabled
    }

    /// The faction settings the world runs with.
    pub fn faction_settings(&self) -> &FactionSettings {
        &self.settings.faction
    }

    /// Java `Player.isGood()`.
    pub fn is_good(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.data.profile.faction == FACTION_GOOD)
    }

    /// Java `Player.isEvil()`.
    pub fn is_evil(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.data.profile.faction == FACTION_EVIL)
    }

    /// Java `Player.setGood()`: good, and no longer evil.
    pub fn set_good(&mut self, id: ObjectId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.data.profile.faction = FACTION_GOOD;
        }
    }

    /// Java `Player.setEvil()`.
    pub fn set_evil(&mut self, id: ObjectId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.data.profile.faction = FACTION_EVIL;
        }
    }

    /// Java `World.getAllGoodPlayers()`, in object id order (D-13).
    pub fn all_good_players(&self) -> Vec<ObjectId> {
        self.players_of_faction(FACTION_GOOD)
    }

    /// Java `World.getAllEvilPlayers()`, in object id order.
    pub fn all_evil_players(&self) -> Vec<ObjectId> {
        self.players_of_faction(FACTION_EVIL)
    }

    fn players_of_faction(&self, faction: i32) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self
            .players
            .values()
            .filter(|p| p.data.profile.faction == faction)
            .map(|p| p.core.object_id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// `(isGood() && other.isEvil()) || (isEvil() && other.isGood())` for two players; the
    /// callers add the `FACTION_SYSTEM_ENABLED` test Java has at each place.
    pub fn factions_oppose(&self, a: ObjectId, b: ObjectId) -> bool {
        (self.is_good(a) && self.is_evil(b)) || (self.is_evil(a) && self.is_good(b))
    }

    /// `(isGood() && other.isGood()) || (isEvil() && other.isEvil())`.
    pub fn same_faction(&self, a: ObjectId, b: ObjectId) -> bool {
        (self.is_good(a) && self.is_good(b)) || (self.is_evil(a) && self.is_evil(b))
    }

    /// Java `NpcTemplate.isClan(name)` of an NPC template id: the template's clan list holds
    /// the generic clan `ALL` or the clan called `name`. False for a template without a clan list.
    pub fn npc_template_is_clan(&self, npc_id: i32, clan_name: &str) -> bool {
        let Some(templates) = self.npc_world.templates_arc() else {
            return false;
        };
        let Some(template) = templates.get(npc_id) else {
            return false;
        };
        if template.clans.is_empty() {
            return false;
        }
        template.clans.contains(&templates.generic_clan_id())
            || template.clans.contains(&templates.clan_id(clan_name))
    }

    /// The faction guard rule of `Guard.isAutoAttackable`, `Guard.onAction`, `Player.isAutoAttackable`
    /// and `AttackableAI.isAggressiveTowards`: with the system and its guards on, an NPC whose
    /// template belongs to the clan named like the other team hates the player.
    pub fn faction_guard_hates(&self, player: ObjectId, npc: ObjectId) -> bool {
        let settings = &self.settings.faction;
        if !settings.enabled || !settings.guards_enabled {
            return false;
        }
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return false;
        };
        (self.is_good(player) && self.npc_template_is_clan(npc_id, &settings.evil_team_name))
            || (self.is_evil(player) && self.npc_template_is_clan(npc_id, &settings.good_team_name))
    }

    /// `Guard.isAutoAttackable(attacker)` for the faction guards: `npc` is a `Guard` and the
    /// playable `attacker` (or its owner) is on the other side.
    pub fn faction_guard_auto_attackable(&self, npc: ObjectId, attacker: ObjectId) -> bool {
        self.npcs.get(&npc).is_some_and(|n| n.class.is_guard())
            && self
                .acting_player(attacker)
                .is_some_and(|player| self.faction_guard_hates(player, npc))
    }

    /// `MapRegionData.getTeleToLocation`: "Faction system base respawn."
    pub(crate) fn faction_base_respawn(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        let settings = &self.settings.faction;
        if !(settings.enabled && settings.respawn_at_base) {
            return None;
        }
        let base = if self.is_good(id) {
            settings.good_base_location
        } else if self.is_evil(id) {
            settings.evil_base_location
        } else {
            return None;
        };
        Some((base.x, base.y, base.z))
    }

    /// The colours and the welcome of `EnterWorld` ("Faction System").
    pub(crate) fn faction_on_enter_world(&mut self, id: ObjectId) {
        if !self.settings.faction.enabled {
            return;
        }
        let settings = &self.settings.faction;
        let (color, team) = if self.is_good(id) {
            (settings.good_name_color, settings.good_team_name.clone())
        } else if self.is_evil(id) {
            (settings.evil_name_color, settings.evil_team_name.clone())
        } else {
            return;
        };
        self.faction_set_colors(id, color);
        let Some(name) = self.players.get(&id).map(|p| p.data.name.clone()) else {
            return;
        };
        let text = format!("Welcome {name}, you are fighting for the {team} faction.");
        self.send_message(id, &text);
        self.send(id, &ExShowScreenMessage::text(&text, 10_000));
        // "for seeing self name color"
        self.send_user_info(id);
    }

    /// `appearance.setNameColor(color); appearance.setTitleColor(color)`. The name colour of the
    /// appearance is the one `//setcolor` sets (`GmFlags::name_color`); the title colour is the
    /// stored `title_color`.
    pub fn faction_set_colors(&mut self, id: ObjectId, color: i32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.gm.name_color = Some(color);
            p.data.profile.title_color = color;
        }
    }

    /// `Player.setTitle(title)`.
    pub fn faction_set_title(&mut self, id: ObjectId, title: &str) {
        if let Some(p) = self.players.get_mut(&id) {
            p.data.title = title.to_owned();
        }
    }

    /// `CharacterSelect`: the balance check of the online players. `faction` is the faction of the
    /// character about to be loaded; the answer is the page to show instead of entering
    /// (`data/html/mods/Faction/ExceededOnlineLimit.htm` with `%more%` and `%less%` replaced), or
    /// `None` when the character may enter.
    pub fn faction_balance_page(&self, faction: i32) -> Option<String> {
        let settings = &self.settings.faction;
        if !(settings.enabled && settings.balance_online_players) {
            return None;
        }
        let good = self.all_good_players().len();
        let evil = self.all_evil_players().len();
        let limit = usize::try_from(settings.balance_player_exceed_limit).unwrap_or(0);
        let (more, less) = if faction == FACTION_GOOD && good >= evil + limit {
            (&settings.good_team_name, &settings.evil_team_name)
        } else if faction == FACTION_EVIL && evil >= good + limit {
            (&settings.evil_team_name, &settings.good_team_name)
        } else {
            return None;
        };
        const PAGE: &str = "data/html/mods/Faction/ExceededOnlineLimit.htm";
        let page = self
            .talk
            .html
            .as_ref()
            .and_then(|cache| cache.get_localized("", PAGE))
            .map_or_else(
                || format!("<html><body>My Text is missing:<br>{PAGE}</body></html>"),
                str::to_owned,
            );
        Some(page.replace("%more%", more).replace("%less%", less))
    }

    /// Java `FactionSystem.broadcastMessageToFaction(name, message)`: `sendMessage` to every
    /// player of that side (the good side when `name` is the good team's name, else the evil one).
    pub fn faction_broadcast_message(&self, faction_name: &str, message: &str) {
        let ids = if faction_name == self.settings.faction.good_team_name {
            self.all_good_players()
        } else {
            self.all_evil_players()
        };
        for id in ids {
            self.send_message(id, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_javas_and_the_system_is_off() {
        let s = FactionSettings::default();
        assert!(!s.enabled);
        assert_eq!(s.good_team_name, "Good");
        assert_eq!(s.evil_name_color, 0x0000FF);
        assert_eq!(s.manager_location.heading, 26808);
        assert_eq!(s.balance_player_exceed_limit, 20);
        assert!(s.guards_enabled && s.respawn_at_base && s.specific_chat && s.balance_online_players);
        assert!(!s.auto_nobless);
    }
}
