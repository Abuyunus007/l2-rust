//! The `Script` bypass handler (Java `ScriptLink`): the quest list of an NPC, a script's talk
//! window and `bypass Script <name> <event>` (`Player.processScriptEvent`).
// Java: dist/game/data/scripts/handlers/bypass/npc/ScriptLink.java
// Java: entity/actor/Player.java (processScriptEvent)
//
// The engine (S-API-01, `script/`) answers first: its scripts by name, and the quest list of
// `bypass Script` alone. `ClanMaster` and `AllianceMaster` are scripts of the engine now
// (S-API-02, `l2-scripts`). The scripts that are still built in (the fortress and castle NPCs,
// the air ship controllers) follow in `process_script_event`; any other name gets the "no quest"
// page Java shows when the script does not exist.

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use crate::world::World;

/// Java `Quest.DEFAULT_NO_QUEST_MSG`.
const DEFAULT_NO_QUEST_MSG: &str = "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>";

/// Java `ScriptLink.onCommand(command, player, target)`, the handler of `Script`.
pub(crate) fn script_link_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    // `command.substring(6).trim()`; a shorter command throws and leaves the name empty.
    let script_name = command.get(6..).unwrap_or("").trim_matches(|c: char| c <= ' ');
    if script_name.is_empty() {
        // `showQuestWindow(player, target.asNpc())`: the engine's quests of the NPC (S-API-01).
        if let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) {
            world.engine_show_quest_window(player, npc);
        }
        return true;
    }
    match script_name.find(' ') {
        None => {
            if let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) {
                world.script_show_quest_window(player, npc, script_name);
            }
        }
        Some(end) => {
            let (name, event) = script_name.split_at(end);
            world.process_script_event(player, name, event.trim_matches(|c: char| c <= ' '));
        }
    }
    true
}

impl World {
    /// Java `ScriptLink.showQuestWindow(player, npc, questId)`.
    fn script_show_quest_window(&mut self, player: ObjectId, npc: ObjectId, script_name: &str) {
        // The scripts of the engine (S-API-01).
        if self.script_show_quest_window_named(player, npc, script_name) {
            return;
        }
        self.show_no_quest_window(player, npc);
    }

    /// `showQuestWindow(player, npc, "")` for a script that does not exist: `noquest.htm` and an
    /// `ActionFailed`.
    fn show_no_quest_window(&mut self, player: ObjectId, npc: ObjectId) {
        let content = self
            .get_htm(player, "data/html/noquest.htm")
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| DEFAULT_NO_QUEST_MSG.to_owned());
        self.insert_object_id_and_show_chat_window(npc, player, &content);
        self.send(player, &ActionFailed);
    }

    /// Java `Player.processScriptEvent(scriptName, event)`.
    fn process_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) {
        // The scripts of the engine (S-API-01).
        if self.script_process_event(player, script_name, event) {
            return;
        }
        // `FortressSiegeManager` (M7-FORT-01, `fort/script.rs`).
        if self.fort_script_event(player, script_name, event) {
            return;
        }
        // The conquerable hall scripts (M7-CHSIEGE-01, `clanhall/siege/`).
        if self.hall_script_event(player, script_name, event) {
            return;
        }
        // The castle scripts (M7-NPC-01, `residence/npc/`).
        if self.residence_script_event(player, script_name, event) {
            return;
        }
        // The wedding priest (M8-MISC-01, `wedding.rs`).
        if self.wedding_script_event(player, script_name, event) {
            return;
        }
        // The air ship controllers (M8-VEHICLE-01, `vehicle/controller.rs`).
        if self.air_ship_script_event(player, script_name, event) {
            return;
        }
    }
}
