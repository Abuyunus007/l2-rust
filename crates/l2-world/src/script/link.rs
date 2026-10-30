//! The `Script` bypass of the engine's scripts: the quest window of an NPC, the quest chooser,
//! and `bypass Script <name> <event>` (Java `Player.processScriptEvent`).
// Java: dist/game/data/scripts/handlers/bypass/npc/ScriptLink.java (showQuestWindow,
//       showQuestChooseWindow, getQuestsForTalk)
// Java: entity/actor/Player.java (processScriptEvent)
//
// The handler itself (`script_link_handler`) is `clan/master_script.rs`, which also knows the
// built-in village master scripts; it asks the engine first.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;

use super::api::ScriptNpc;
use super::ctx::no_quest_msg;
use super::setup::{EventKind, ScriptId};
use crate::dialog::INTERACTION_DISTANCE;
use crate::target::distance_2d;
use crate::world::World;

/// `ScriptLink.TO_LEAD_AND_BE_LED`.
const TO_LEAD_AND_BE_LED: i32 = 118;
/// `ScriptLink.THE_LEADER_AND_THE_FOLLOWER`.
const THE_LEADER_AND_THE_FOLLOWER: i32 = 123;
/// The tutorial quest: never listed in the chooser.
const TUTORIAL_QUEST_ID: i32 = 255;

/// A quest id of the journal: 1 to 19999.
pub(crate) fn is_journal_quest(id: i32) -> bool {
    id > 0 && id < 20000
}

impl World {
    fn script_npc_target(&self, npc: ObjectId) -> Option<ScriptNpc> {
        self.npcs.get(&npc).map(|n| ScriptNpc {
            object_id: npc,
            id: n.npc_id,
        })
    }

    /// Java `ScriptLink.showQuestWindow(player, npc, questId)` for a script the engine has:
    /// `true` when it had one of that name (else the caller goes on with its own).
    pub(crate) fn script_show_quest_window_named(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        name: &str,
    ) -> bool {
        let Some(script) = self.scripts.by_name(name) else {
            return false;
        };
        self.script_quest_window(player, npc, Some(script));
        true
    }

    /// Java `ScriptLink.showQuestWindow(player, npc, questId)`: `None` is a quest that does not
    /// exist (the "no quest" page).
    fn script_quest_window(&mut self, player: ObjectId, npc: ObjectId, script: Option<ScriptId>) {
        let Some(n) = self.script_npc_target(npc) else {
            return;
        };
        let mut content = None;
        match script {
            Some(script) => {
                let quest_id = self.scripts.quest_id_of(script);
                let name = self.scripts.name_of(script);
                let has_state = self
                    .scripts
                    .players
                    .get(&player)
                    .is_some_and(|q| q.contains(name));
                let (weight_penalty, under_90) = (
                    self.players.get(&player).map_or(0, |p| p.weight_penalty),
                    self.is_inventory_under_90(player, true),
                );
                if is_journal_quest(quest_id) && (weight_penalty >= 3 || !under_90) {
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::UnableToProcessThisRequestUntilYourInventorySWeightAndSlotCountAreLessThan80PercentOfCapacity,
                        ),
                    );
                    return;
                }
                if !has_state && is_journal_quest(quest_id) && self.all_active_quests(player).len() > 40 {
                    // `new NpcHtmlMessage(npcObjId)` + `setFile(player, "data/html/fullquest.html")`.
                    if let Some(html) = self.html_from_file(player, npc.0, "data/html/fullquest.html") {
                        self.send_html(player, &html);
                    }
                    return;
                }
                self.script_notify_talk(script, n, player);
            }
            None => content = Some(no_quest_msg(self, player)),
        }
        if let Some(content) = content {
            self.insert_object_id_and_show_chat_window(npc, player, &content);
        }
        self.send(player, &ActionFailed);
    }

    /// Java `ScriptLink.showQuestWindow(player, npc)`: the quest of the NPC the player can talk
    /// about, the chooser when there are several, or the "no quest" page.
    pub(crate) fn engine_show_quest_window(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(n) = self.script_npc_target(npc) else {
            return;
        };
        let mut condition_meet = false;
        // NOTE(java-quirk): Java collects the quests in a `HashSet`, so the chooser lists them in
        // hash order; here they come in the order found (talk states, then start NPC).
        let mut options: Vec<ScriptId> = Vec::new();
        // `getQuestsForTalk`: the quests with an `ON_NPC_TALK` listener the player has a state of.
        let talk: Vec<ScriptId> = self.scripts.listeners.of(EventKind::NpcTalk, n.id).to_vec();
        for script in talk {
            let name = self.scripts.name_of(script);
            if !self
                .scripts
                .players
                .get(&player)
                .is_some_and(|q| q.contains(name))
            {
                continue;
            }
            if is_journal_quest(self.scripts.quest_id_of(script)) {
                if !options.contains(&script) {
                    options.push(script);
                }
                if self.can_start_quest(script, player) {
                    condition_meet = true;
                }
            }
        }
        let start: Vec<ScriptId> = self.scripts.listeners.of(EventKind::NpcQuestStart, n.id).to_vec();
        for script in start {
            if is_journal_quest(self.scripts.quest_id_of(script)) {
                if !options.contains(&script) {
                    options.push(script);
                }
                if self.can_start_quest(script, player) {
                    condition_meet = true;
                }
            }
        }

        let apprentice_online = self.apprentice_online(player);
        if !condition_meet {
            self.script_quest_window(player, npc, None);
        } else if options.len() > 1
            || (apprentice_online
                && options
                    .iter()
                    .any(|s| self.scripts.quest_id_of(*s) == TO_LEAD_AND_BE_LED))
        {
            self.script_show_quest_choose_window(player, npc, &options);
        } else if let Some(&only) = options.first() {
            self.script_quest_window(player, npc, Some(only));
        } else {
            self.script_quest_window(player, npc, None);
        }
    }

    /// `player.getApprentice() > 0 && World.getPlayer(player.getApprentice()) != null`.
    fn apprentice_online(&self, player: ObjectId) -> bool {
        let apprentice = self.player_clan_state(player).apprentice;
        apprentice > 0 && self.players.contains_key(&ObjectId(apprentice))
    }

    /// Java `ScriptLink.showQuestChooseWindow(player, npc, quests)`.
    fn script_show_quest_choose_window(&mut self, player: ObjectId, npc: ObjectId, quests: &[ScriptId]) {
        let mut sb = String::with_capacity(128);
        sb.push_str("<html><body>");
        let apprentice_online = self.apprentice_online(player);
        for &quest in quests {
            let Some(script) = self.scripts.script(quest) else {
                continue;
            };
            // Skip tutorial links.
            let quest_id = script.id();
            if quest_id == TUTORIAL_QUEST_ID {
                continue;
            }
            let custom = script.is_custom();
            let qs = self
                .scripts
                .players
                .get(&player)
                .and_then(|q| q.get(script.name()));
            let state = match qs {
                None => {
                    if custom {
                        ""
                    } else {
                        "01"
                    }
                }
                Some(qs) if qs.is_created() => {
                    if custom {
                        ""
                    } else {
                        "01"
                    }
                }
                Some(qs) if qs.is_started() => {
                    if custom {
                        " (In Progress)"
                    } else {
                        "02"
                    }
                }
                Some(qs) if qs.is_completed() => {
                    if custom {
                        " (Done)"
                    } else {
                        "03"
                    }
                }
                // NOTE(java-quirk): an unknown state keeps the text of the quest before it.
                Some(_) => "",
            };
            sb.push_str(&format!(
                "<a action=\"bypass npc_{}_Script {}\">",
                npc.0,
                script.name()
            ));
            sb.push('[');
            if custom {
                sb.push_str(script.path());
                sb.push_str(state);
            } else {
                // The multilingual branch (`MULTILANG_ENABLE`) is M9.
                sb.push_str(&format!("<fstring>{}{state}</fstring>", script.npc_string_id()));
            }
            sb.push_str("]</a><br>");
            if apprentice_online {
                let sponsor = match quest_id {
                    TO_LEAD_AND_BE_LED => Some("Q00118_ToLeadAndBeLed"),
                    THE_LEADER_AND_THE_FOLLOWER => Some("Q00123_TheLeaderAndTheFollower"),
                    _ => None,
                };
                if let Some(name) = sponsor {
                    sb.push_str(&format!(
                        "<a action=\"bypass Script {name} sponsor\">[<fstring>{quest_id}{state}</fstring> (Sponsor)]</a><br>"
                    ));
                }
            }
        }
        sb.push_str("</body></html>");
        // Send a Server->Client packet NpcHtmlMessage to the Player in order to display the
        // message of the Npc.
        self.insert_object_id_and_show_chat_window(npc, player, &sb);
    }

    /// The rows of `GmViewQuestInfo`: the id and the cond of each active quest of the player.
    pub fn gm_view_quests(&self, player: ObjectId) -> Vec<(i32, i32)> {
        let Some(quests) = self.scripts.players.get(&player) else {
            return Vec::new();
        };
        self.all_active_quests(player)
            .into_iter()
            .map(|script| {
                let cond = quests.get(self.scripts.name_of(script)).map_or(0, |qs| qs.cond());
                (self.scripts.quest_id_of(script), cond)
            })
            .collect()
    }

    /// Java `Player.getAllActiveQuests()`: the started quests with a journal id, in the order of
    /// the player's quest map.
    pub(crate) fn all_active_quests(&self, player: ObjectId) -> Vec<ScriptId> {
        let Some(quests) = self.scripts.players.get(&player) else {
            return Vec::new();
        };
        quests
            .iter()
            .filter(|qs| qs.is_started())
            .filter_map(|qs| self.scripts.by_name(qs.quest_name()))
            .filter(|s| is_journal_quest(self.scripts.quest_id_of(*s)))
            .collect()
    }

    /// Java `Player.processScriptEvent(scriptName, event)` for a script the engine has: `true`
    /// when it had one of that name.
    pub(crate) fn script_process_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        let Some(script) = self.scripts.by_name(script_name) else {
            return false;
        };
        if event.is_empty() {
            return true;
        }
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return true;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            // `isInsideRadius2D`: `<=`.
            (Some(me), Some(pos)) => distance_2d(me, pos) <= f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let target = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => Some(target),
            _ if quest_npc > 0 => self
                .find_object(ObjectId(quest_npc))
                .filter(|o| self.npcs.contains_key(o))
                .filter(|npc| in_range(self, *npc)),
            _ => None,
        };
        if let Some(npc) = target.and_then(|t| self.script_npc_target(t)) {
            self.script_notify_event(script, event, Some(npc), Some(player));
        }
        true
    }

    /// The `item_<objectId>_<command>` bypass of an item page: Java `Item.onBypassFeedback`.
    pub(crate) fn script_item_bypass(&mut self, player: ObjectId, object_id: i32, command: &str) {
        let Some(item) = self
            .inventory_item(player, ObjectId(object_id))
            .map(|i| super::ScriptItem {
                object_id: i.object_id,
                id: i.item_id,
            })
        else {
            return;
        };
        self.script_on_item_bypass(player, item, command);
    }
}

/// Java `Quest.getNpcStringId()`: the client string of the quest's name.
pub(crate) fn npc_string_id(quest_id: i32) -> i32 {
    if quest_id > 10000 {
        quest_id - 5000
    } else {
        quest_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_string_ids_shift_above_10000() {
        assert_eq!(npc_string_id(1), 1);
        assert_eq!(npc_string_id(10000), 10000);
        assert_eq!(npc_string_id(10001), 5001);
    }
}
