//! The `Script` bypass and the two village master scripts that list the clan and alliance
//! commands: `ClanMaster` and `AllianceMaster` (M6-CLAN-02).
// Java: dist/game/data/scripts/handlers/bypass/npc/ScriptLink.java
// Java: dist/game/data/scripts/village_master/ClanMaster/ClanMaster.java
// Java: dist/game/data/scripts/village_master/AllianceMaster/AllianceMaster.java
// Java: entity/actor/Player.java (processScriptEvent), mechanics/script/Quest.java (notifyTalk,
//       notifyEvent, showResult, showHtmlFile, getHtm)
//
// # What is ported
// A village master's page (`data/html/villagemaster/<npcId>.htm`) links to
// `bypass -h Script ClanMaster` and `bypass -h Script AllianceMaster`. The first opens the
// script's `onTalk` page (`9000-01.htm` / `9001-01.htm`), the pages after it send
// `bypass Script <Name> <page>.htm`, which is the script's `onEvent`; both show the page from
// `data/scripts/village_master/<Name>/`. The pages that need a clan leader or a clan show the
// "no" page instead. The commands on those pages are `npc_<id>_create_clan ...` and so on; they
// are `village_master.rs`.
//
// D-05: the script API (quests, event listeners) comes last. Until then `ScriptLink` knows these
// two scripts by name; any other name gets the "no quest" page Java shows when the script does
// not exist, and the quest list (`bypass Script` alone) is empty.

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::debug;

use crate::dialog::INTERACTION_DISTANCE;
use crate::target::distance_2d;
use crate::world::World;

/// Java `Quest.DEFAULT_NO_QUEST_MSG`.
const DEFAULT_NO_QUEST_MSG: &str = "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>";

/// `ClanMaster.LEADER_REQUIRED`: the page, and the page a player who is not a clan leader gets.
const CLAN_MASTER_LEADER_REQUIRED: [(&str, &str); 11] = [
    ("9000-03.htm", "9000-03-no.htm"),
    ("9000-04.htm", "9000-04-no.htm"),
    ("9000-05.htm", "9000-05-no.htm"),
    ("9000-07.htm", "9000-07-no.htm"),
    ("9000-12a.htm", "9000-07-no.htm"),
    ("9000-12b.htm", "9000-07-no.htm"),
    ("9000-13a.htm", "9000-07-no.htm"),
    ("9000-13b.htm", "9000-07-no.htm"),
    ("9000-14a.htm", "9000-07-no.htm"),
    ("9000-14b.htm", "9000-07-no.htm"),
    ("9000-15.htm", "9000-07-no.htm"),
];

/// A script with pages (the part of a Java `Script` these two use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MasterScript {
    ClanMaster,
    AllianceMaster,
}

impl MasterScript {
    /// Java `ScriptManager.getScript(name)`: the names are case sensitive.
    fn by_name(name: &str) -> Option<Self> {
        match name {
            "ClanMaster" => Some(Self::ClanMaster),
            "AllianceMaster" => Some(Self::AllianceMaster),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::ClanMaster => "ClanMaster",
            Self::AllianceMaster => "AllianceMaster",
        }
    }

    /// Java `Quest.getPath()`: the folder below `data/scripts`.
    fn path(self) -> &'static str {
        match self {
            Self::ClanMaster => "village_master/ClanMaster",
            Self::AllianceMaster => "village_master/AllianceMaster",
        }
    }

    /// `onTalk`.
    fn on_talk(self) -> &'static str {
        match self {
            Self::ClanMaster => "9000-01.htm",
            Self::AllianceMaster => "9001-01.htm",
        }
    }
}

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
        // `showQuestWindow(player, target.asNpc())`: no quests exist yet (D-05), so the player
        // is told there is none for this NPC.
        if let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) {
            world.show_no_quest_window(player, npc);
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
        match MasterScript::by_name(script_name) {
            Some(script) => {
                self.notify_talk(script, player, npc);
                // "Send a Server->Client ActionFailed to the Player in order to avoid that the
                // client wait another packet."
                self.send(player, &ActionFailed);
            }
            None => self.show_no_quest_window(player, npc),
        }
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

    /// Java `Quest.notifyTalk(npc, player)`.
    fn notify_talk(&mut self, script: MasterScript, player: ObjectId, npc: ObjectId) {
        let result = script.on_talk();
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
        self.show_script_result(script, player, Some(result), npc);
    }

    /// Java `Player.processScriptEvent(scriptName, event)`.
    fn process_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) {
        let Some(script) = MasterScript::by_name(script_name) else {
            return;
        };
        if event.is_empty() {
            return;
        }
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return;
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
                // `World.findObject(getLastQuestNpcObject())`, `isNpc()`
                match self
                    .find_object(ObjectId(quest_npc))
                    .filter(|o| self.npcs.contains_key(o))
                {
                    Some(npc) if in_range(self, npc) => npc,
                    _ => return,
                }
            }
            _ => return,
        };
        // `Quest.notifyEvent`: `onEvent`, then `showResult`.
        let result = self.script_on_event(script, event, player);
        self.show_script_result(script, player, Some(&result), npc);
    }

    /// `ClanMaster.onEvent` and `AllianceMaster.onEvent`.
    fn script_on_event(&self, script: MasterScript, event: &str, player: ObjectId) -> String {
        match script {
            MasterScript::ClanMaster => {
                if let Some((_, no_page)) = CLAN_MASTER_LEADER_REQUIRED
                    .iter()
                    .find(|(page, _)| *page == event)
                    && !self.is_clan_leader(player)
                {
                    return (*no_page).to_owned();
                }
                event.to_owned()
            }
            MasterScript::AllianceMaster => {
                if event != "9001-01.htm" && self.clan_of(player).is_none() {
                    return "9001-04.htm".to_owned();
                }
                event.to_owned()
            }
        }
    }

    /// Java `Quest.showResult(player, res, npc)` for a script that is not a quest.
    fn show_script_result(
        &mut self,
        script: MasterScript,
        player: ObjectId,
        res: Option<&str>,
        npc: ObjectId,
    ) {
        let Some(res) = res.filter(|r| !r.is_empty()) else {
            return;
        };
        if res.ends_with(".htm") || res.ends_with(".html") {
            // `Quest.getHtm`: the script's folder, then `data/scripts/quests/<name>`.
            let found = [
                format!("data/scripts/{}/{res}", script.path()),
                format!("data/scripts/quests/{}/{res}", script.name()),
            ]
            .iter()
            .find_map(|path| self.get_htm(player, path));
            match found {
                Some(content) => self.send_script_html(player, npc, &content),
                None => debug!("{}: page {res} of {} not found.", player, script.name()),
            }
        } else if res.starts_with("<html") {
            self.send_script_html(player, npc, res);
        } else {
            self.send_message(player, res);
        }
    }

    /// `showHtmlFile`'s send: `%objectId%` and `%playername%` replaced, then `ActionFailed`.
    fn send_script_html(&mut self, player: ObjectId, npc: ObjectId, content: &str) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let html = content
            .replace("%objectId%", &npc.0.to_string())
            .replace("%playername%", &name);
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }
}
