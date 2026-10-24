//! The two fortress scripts: `FortressSiegeManager` (the suspicious merchant who registers a clan
//! for the siege) and `FortressArcherCaptain`.
// Java: dist/game/data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager.java
// Java: dist/game/data/scripts/ai/others/FortressArcherCaptain/FortressArcherCaptain.java
// Java: mechanics/script/Quest.java (showResult, showHtmlFile, getHtm)
// Java: entity/actor/Player.java (processScriptEvent)
//
// D-05: the script API comes last, so these two are built in like `ClanMaster` (`clan/
// master_script.rs`): the `Script` bypass hands the command to [`World::fort_script_event`],
// the first click on the NPC to [`World::fort_script_first_talk`]. The pages are the script's
// own (`data/scripts/ai/others/<Name>/*.html`).
//
// Registering and cancelling read the fortress's registrations again before they act (D-50), so
// the clan list this world holds is the one the database has (a registration made on this
// world without a row stays, see `FortSiegeClan::written`).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, warn};

use super::FortThen;
use crate::dialog::INTERACTION_DISTANCE;
use crate::residence::{FortId, ResidenceRef};
use crate::target::distance_2d;
use crate::world::World;

/// `FortressSiegeManager.MANAGERS`: the suspicious merchant of each fortress.
const MANAGERS: [i32; 21] = [
    35659, 35690, 35728, 35759, 35797, 35828, 35859, 35897, 35928, 35966, 36004, 36035, 36073, 36111, 36142,
    36173, 36211, 36249, 36287, 36318, 36356,
];

/// `FortressArcherCaptain.ARCHER_CAPTAIN`.
const ARCHER_CAPTAINS: [i32; 21] = [
    35661, 35692, 35730, 35761, 35799, 35830, 35861, 35899, 35930, 35968, 36006, 36037, 36075, 36113, 36144,
    36175, 36213, 36251, 36289, 36320, 36358,
];

const MANAGER_PATH: &str = "data/scripts/ai/others/FortressSiegeManager";
const CAPTAIN_PATH: &str = "data/scripts/ai/others/FortressArcherCaptain";

/// The pages `onEvent` passes through (`case "FortressSiegeManager-11.html": ...`).
const PASS_THROUGH: [&str; 5] = [
    "FortressSiegeManager-11.html",
    "FortressSiegeManager-13.html",
    "FortressSiegeManager-14.html",
    "FortressSiegeManager-15.html",
    "FortressSiegeManager-16.html",
];

/// What a script returns to `Quest.showResult`.
enum Reply {
    /// A page of the script (`"FortressSiegeManager-03.html"`).
    Page(&'static str),
    /// A page by name that is not a constant.
    Html(String),
}

impl World {
    /// Java `onFirstTalk` of the two scripts. `false` for any other NPC.
    pub(crate) fn fort_script_first_talk(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        if MANAGERS.contains(&npc_id) {
            self.fort_manager_script_first_talk(npc, player);
            return true;
        }
        if ARCHER_CAPTAINS.contains(&npc_id) {
            // `(player.getClan() != null) && (player.getClanId() == owner)`.
            let owner = self
                .npc_fort(npc)
                .and_then(|f| self.fort(f))
                .map_or(0, |f| f.owner_id);
            let own = self.clan_of(player).is_some() && self.clan_id_of(player) == owner;
            let page = if own {
                "FortressArcherCaptain.html"
            } else {
                "FortressArcherCaptain-01.html"
            };
            self.fort_script_show_page(CAPTAIN_PATH, page, npc, player);
            return true;
        }
        false
    }

    /// `FortressSiegeManager.onFirstTalk`.
    fn fort_manager_script_first_talk(&mut self, npc: ObjectId, player: ObjectId) {
        let Some(fort) = self.npc_fort(npc) else {
            return;
        };
        let owner = self.fort(fort).map_or(0, |f| f.owner_id);
        if owner == 0 {
            self.fort_script_show_page(MANAGER_PATH, "FortressSiegeManager.html", npc, player);
            return;
        }
        let clan_name = self.clan(owner).map(|c| c.name.clone()).unwrap_or_default();
        let Some(content) = self.get_htm(player, &format!("{MANAGER_PATH}/FortressSiegeManager-01.html"))
        else {
            return;
        };
        let html = content
            .replace("%clanName%", &clan_name)
            .replace("%objectId%", &npc.0.to_string());
        self.fort_script_show_html(player, npc, &html);
    }

    /// `Quest.showResult(player, "<page>.html", npc)` / `showHtmlFile`: the page of the script,
    /// `%objectId%` replaced, then `ActionFailed`.
    fn fort_script_show_page(&mut self, path: &str, page: &str, npc: ObjectId, player: ObjectId) {
        let Some(content) = self.get_htm(player, &format!("{path}/{page}")) else {
            debug!("{player}: page {page} of {path} not found.");
            return;
        };
        let html = content.replace("%objectId%", &npc.0.to_string());
        self.fort_script_show_html(player, npc, &html);
    }

    /// `NpcHtmlMessage(npcObjId, html)` with `%playername%` and an `ActionFailed`.
    fn fort_script_show_html(&mut self, player: ObjectId, npc: ObjectId, html: &str) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let html = html.replace("%playername%", &name);
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }

    /// `Player.processScriptEvent(scriptName, event)` for `FortressSiegeManager`. `false` when the
    /// script is another one.
    pub(crate) fn fort_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        if script_name != "FortressSiegeManager" {
            return false;
        }
        if event.is_empty() {
            return true;
        }
        // The NPC the player talks to: the last folk, else the last quest NPC, in reach.
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return true;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let npc = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => target,
            _ if quest_npc > 0 => {
                match self
                    .find_object(ObjectId(quest_npc))
                    .filter(|o| self.npcs.contains_key(o))
                {
                    Some(npc) if in_range(self, npc) => npc,
                    _ => return true,
                }
            }
            _ => return true,
        };
        // `Quest.notifyEvent`: `onEvent`, then `showResult`.
        if let Some(page) = PASS_THROUGH.iter().find(|p| **p == event) {
            self.fort_script_show_page(MANAGER_PATH, page, npc, player);
            return true;
        }
        let then = match event {
            "register" => FortThen::Register { player, npc },
            "cancel" => FortThen::Cancel { player, npc },
            "warInfo" => FortThen::WarInfo { player, npc },
            _ => return true,
        };
        match self.npc_fort(npc) {
            // D-50: the registrations are read again first.
            Some(fort) => self.fort_request_siege_reread(fort, then),
            None => warn!("{player}: the siege manager {npc} stands at no fortress."),
        }
        true
    }

    /// The `onEvent` of `FortressSiegeManager` once the rows are fresh.
    pub(super) fn fort_script_after_reread(&mut self, then: FortThen) {
        let (player, npc) = match then {
            FortThen::Register { player, npc }
            | FortThen::Cancel { player, npc }
            | FortThen::WarInfo { player, npc } => (player, npc),
            FortThen::Logistics { player, npc, kind } => {
                self.fort_logistics_command(npc, player, kind);
                return;
            }
        };
        if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
            return;
        }
        let Some(fort) = self.npc_fort(npc) else {
            return;
        };
        let reply = match then {
            FortThen::Register { .. } => self.fort_script_register(player, npc, fort),
            FortThen::Cancel { .. } => self.fort_script_cancel(player, fort),
            _ => Some(Reply::Page(if self.fort_attacker_clans(fort).is_empty() {
                "FortressSiegeManager-20.html"
            } else {
                "FortressSiegeManager-21.html"
            })),
        };
        match reply {
            Some(Reply::Page(page)) => self.fort_script_show_page(MANAGER_PATH, page, npc, player),
            Some(Reply::Html(html)) => self.fort_script_show_html(player, npc, &html),
            None => {}
        }
    }

    /// `case "register"`.
    fn fort_script_register(&mut self, player: ObjectId, npc: ObjectId, fort: FortId) -> Option<Reply> {
        let Some(clan) = self
            .clan_of(player)
            .map(|c| (c.id, c.level, c.castle_id, c.fort_id))
        else {
            return Some(Reply::Page("FortressSiegeManager-02.html"));
        };
        let (clan_id, level, clan_castle, clan_fort) = clan;
        if clan_fort == fort.0 {
            return self.fort_script_owner_page(fort);
        }
        if !self.fort_has_siege_access(player) {
            return Some(Reply::Page("FortressSiegeManager-10.html"));
        }
        let min_level = self.fort.settings.siege.siege_clan_min_level;
        if level < min_level {
            return Some(Reply::Page("FortressSiegeManager-04.html"));
        }
        // NOTE(java-quirk): `npc.getCastle().getResidenceId()` fails for an NPC that has no
        // castle; nothing is shown then.
        let castle = self.npc_castle(npc)?.0;
        let state = self.fort(fort).map_or(0, |f| f.state);
        if clan_castle == castle && state == 2 {
            return Some(Reply::Page("FortressSiegeManager-18.html"));
        }
        if clan_castle != 0 && clan_castle != castle && self.fort.settings.siege.just_to_territory {
            return Some(Reply::Page("FortressSiegeManager-17.html"));
        }
        let rebel = self.fort_time_till_rebel_army(fort);
        if rebel > 0 && rebel <= 7200 {
            return Some(Reply::Page("FortressSiegeManager-19.html"));
        }
        let _ = clan_id;
        match self.fort_add_attacker(fort, player, true) {
            1 => Some(Reply::Page("FortressSiegeManager-03.html")),
            2 => Some(Reply::Page("FortressSiegeManager-07.html")),
            3 => Some(Reply::Page("FortressSiegeManager-06.html")),
            4 => {
                let name = self.residence_name(ResidenceRef::Fort(fort));
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YourClanHasBeenRegisteredToS1SFortressBattle)
                        .with(SystemMessageParam::Text(name)),
                );
                Some(Reply::Page("FortressSiegeManager-05.html"))
            }
            _ => None,
        }
    }

    /// `case "cancel"`.
    fn fort_script_cancel(&mut self, player: ObjectId, fort: FortId) -> Option<Reply> {
        let Some((clan_id, clan_fort)) = self.clan_of(player).map(|c| (c.id, c.fort_id)) else {
            return Some(Reply::Page("FortressSiegeManager-02.html"));
        };
        if clan_fort == fort.0 {
            return self.fort_script_owner_page(fort);
        }
        if !self.fort_has_siege_access(player) {
            return Some(Reply::Page("FortressSiegeManager-10.html"));
        }
        // `FortSiegeManager.checkIsRegistered`.
        if !self.fort_check_is_attacker(fort, clan_id) {
            return Some(Reply::Page("FortressSiegeManager-09.html"));
        }
        self.fort_remove_attacker(fort, clan_id);
        Some(Reply::Page("FortressSiegeManager-08.html"))
    }

    /// The page "Well... Member of the clan %clanName% of repute!": `html.getHtml()` with the
    /// owner's name, returned as the raw html.
    fn fort_script_owner_page(&self, fort: FortId) -> Option<Reply> {
        let owner = self.fort(fort).map(|f| f.owner_id)?;
        // `fortress.getOwnerClan().getName()` fails without an owner.
        let name = self.clan(owner)?.name.clone();
        let content = self.get_htm(
            ObjectId(0),
            &format!("{MANAGER_PATH}/FortressSiegeManager-12.html"),
        )?;
        Some(Reply::Html(content.replace("%clanName%", &name)))
    }
}
