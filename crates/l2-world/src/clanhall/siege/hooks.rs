//! Where the six scripts meet the world: the `Script` bypass, the first click on a script's NPC
//! and the NPC events the scripts listen to, and `Quest.showResult` for what a script returns.
// Java: mechanics/script/Quest.java (notifyEvent, notifyFirstTalk, showResult, showHtmlFile,
//       getHtm), entity/actor/Player.java (processScriptEvent)
// Java: entity/actor/Attackable.java (addDamage: OnAttackableAttack; doDie: OnAttackableKill after
//       `_onKillDelay`), entity/actor/Npc.java (onSpawn: OnNpcSpawn)
// Java: mechanics/events (the listeners the scripts register with `addAttackId`, `addKillId`,
//       `addSpawnId`, `addSkillSeeId`, `addFirstTalkId`, `addStartNpc`, `addTalkId`)
//
// D-05: the script API comes last. These six scripts are built in like `ClanMaster` and the two
// fortress scripts: the tables below say which NPC each listens to.

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::{debug, warn};

use super::{
    BANDIT_STRONGHOLD, BEAST_FARM, DEVASTATED_CASTLE, FORTRESS_OF_DEAD, FORTRESS_RESISTANCE, HallScript,
    HallSiegeTimer, RAINBOW_SPRINGS, dead, devastated, rainbow, resistance,
};
use crate::dialog::INTERACTION_DISTANCE;
use crate::residence::ClanHallId;
use crate::target::distance_2d;
use crate::world::World;

/// `Attackable._onKillDelay`.
const KILL_DELAY_MS: i64 = 2500;

/// The NPC ids of Bandit Stronghold and Wild Beast Reserve's warriors and teleporters.
const BANDIT_ALLIES: [i32; 5] = [35428, 35429, 35430, 35431, 35432];
const WILD_ALLIES: [i32; 5] = [35618, 35619, 35620, 35621, 35622];
const BANDIT_MESSENGER: i32 = 35437;
const WILD_MESSENGER: i32 = 35627;
const BANDIT_TELEPORT_1: i32 = 35560;
const WILD_TELEPORT_1: i32 = 35612;

/// The script an NPC listens for `addAttackId`.
fn attack_script(npc_id: i32) -> Option<HallScript> {
    match npc_id {
        resistance::BLOODY_LORD_NURKA => Some(HallScript::FortressOfResistance),
        devastated::GUSTAV => Some(HallScript::DevastatedCastle),
        dead::LIDIA => Some(HallScript::FortressOfTheDead),
        _ => None,
    }
}

/// The script an NPC listens for `addKillId`.
fn kill_script(npc_id: i32) -> Option<HallScript> {
    match npc_id {
        resistance::BLOODY_LORD_NURKA => Some(HallScript::FortressOfResistance),
        devastated::GUSTAV => Some(HallScript::DevastatedCastle),
        dead::LIDIA | dead::ALFRED | dead::GISELLE => Some(HallScript::FortressOfTheDead),
        id if BANDIT_ALLIES.contains(&id) => Some(HallScript::BanditStronghold),
        id if WILD_ALLIES.contains(&id) => Some(HallScript::WildBeastReserve),
        id if rainbow::GOURDS.contains(&id) || id == rainbow::CHEST => {
            Some(HallScript::RainbowSpringsChateau)
        }
        _ => None,
    }
}

/// The script an NPC listens for `addSpawnId`.
fn spawn_script(npc_id: i32) -> Option<HallScript> {
    match npc_id {
        devastated::MIKHAIL | devastated::DIETRICH => Some(HallScript::DevastatedCastle),
        dead::LIDIA | dead::ALFRED | dead::GISELLE => Some(HallScript::FortressOfTheDead),
        id if BANDIT_ALLIES.contains(&id) => Some(HallScript::BanditStronghold),
        id if WILD_ALLIES.contains(&id) => Some(HallScript::WildBeastReserve),
        id if rainbow::GOURDS.contains(&id) || id == rainbow::ENRAGED_YETI => {
            Some(HallScript::RainbowSpringsChateau)
        }
        _ => None,
    }
}

/// The script an NPC listens for `addFirstTalkId`, and whether it is also a start NPC
/// (`addStartNpc`: the player's last quest NPC is set).
fn first_talk_script(npc_id: i32) -> Option<(HallScript, bool)> {
    match npc_id {
        resistance::MESSENGER => Some((HallScript::FortressOfResistance, false)),
        BANDIT_MESSENGER => Some((HallScript::BanditStronghold, true)),
        id if (BANDIT_TELEPORT_1..BANDIT_TELEPORT_1 + 6).contains(&id) => {
            Some((HallScript::BanditStronghold, false))
        }
        WILD_MESSENGER => Some((HallScript::WildBeastReserve, true)),
        id if (WILD_TELEPORT_1..WILD_TELEPORT_1 + 6).contains(&id) => {
            Some((HallScript::WildBeastReserve, false))
        }
        rainbow::MESSENGER | rainbow::CARETAKER => Some((HallScript::RainbowSpringsChateau, false)),
        id if rainbow::YETIS.contains(&id) => Some((HallScript::RainbowSpringsChateau, false)),
        _ => None,
    }
}

impl World {
    /// Whether the hall of the script has its engine (the script was loaded).
    fn hall_script_loaded(&self, script: HallScript) -> bool {
        self.hall_has_siege(ClanHallId(script.hall_id()))
    }

    /// `HtmCache.getHtm` with Java's "My Text is missing" page for a file that is not there (what
    /// `NpcHtmlMessage.setFile` shows).
    pub(super) fn hall_page_text(&self, player: ObjectId, path: &str) -> String {
        match self.get_htm(player, path) {
            Some(html) => html,
            None => {
                warn!("NpcHtmlMessage: Missing html page {path}");
                format!("<html><body>My Text is missing:<br>{path}</body></html>")
            }
        }
    }

    /// Java `Quest.showHtmlFile(player, filename, npc)`: the page of the script (`getHtm`), with
    /// `%objectId%` and `%playername%` replaced, then `ActionFailed`.
    fn hall_show_html_file(
        &mut self,
        script: HallScript,
        player: ObjectId,
        filename: &str,
        npc: Option<ObjectId>,
    ) {
        let name = script.name();
        let candidates = [
            if filename.starts_with("data/") {
                filename.to_owned()
            } else {
                format!("data/scripts/conquerablehalls/{name}/{filename}")
            },
            format!("data/scripts/conquerablehalls/{name}/{filename}"),
            format!("data/scripts/quests/{name}/{filename}"),
        ];
        let Some(mut content) = candidates.iter().find_map(|p| self.get_htm(player, p)) else {
            return;
        };
        if let Some(npc) = npc {
            content = content.replace("%objectId%", &npc.0.to_string());
        }
        let player_name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        content = content.replace("%playername%", &player_name);
        if let Some(message) = NpcHtmlMessage::new(npc.map_or(0, |n| n.0), &content) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }

    /// Java `Quest.showResult(player, res, npc)`.
    pub(super) fn hall_show_result(
        &mut self,
        script: HallScript,
        player: ObjectId,
        res: Option<&str>,
        npc: Option<ObjectId>,
    ) {
        let Some(res) = res.filter(|r| !r.is_empty()) else {
            return;
        };
        if res.ends_with(".htm") || res.ends_with(".html") {
            self.hall_show_html_file(script, player, res, npc);
        } else if res.starts_with("<html") {
            let name = self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            let html = res.replace("%playername%", &name);
            if let Some(message) = NpcHtmlMessage::new(npc.map_or(0, |n| n.0), &html) {
                self.send_html(player, &message);
            }
            self.send(player, &ActionFailed);
        } else {
            self.send_message(player, res);
        }
    }

    // ------------------------------------------------------------------ bypass Script

    /// Java `Player.processScriptEvent(scriptName, event)` for the hall scripts. `false` when the
    /// script is another one.
    pub(crate) fn hall_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        let Some(script) = HallScript::by_name(script_name) else {
            return false;
        };
        if !self.hall_script_loaded(script) {
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
        let hall = ClanHallId(script.hall_id());
        // D-50: the registrations of the flag wars and of Rainbow Springs change on the Java game
        // server: read them again before the messenger acts (a page needs no row).
        let registration = matches!(
            script,
            HallScript::BanditStronghold | HallScript::WildBeastReserve | HallScript::RainbowSpringsChateau
        );
        if registration && !event.ends_with(".htm") {
            self.request_residence_reread(
                crate::residence::ResidenceRef::ClanHall(hall),
                crate::residence::ResidenceReadThen::HallSiege(super::HallThen::ScriptEvent {
                    player,
                    npc,
                    script,
                    event: event.to_owned(),
                }),
            );
            return true;
        }
        self.hall_script_run_event(script, player, npc, event);
        true
    }

    /// `Quest.notifyEvent`: `onEvent`, then `showResult`.
    pub(crate) fn hall_script_run_event(
        &mut self,
        script: HallScript,
        player: ObjectId,
        npc: ObjectId,
        event: &str,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let result = match script {
            HallScript::BanditStronghold | HallScript::WildBeastReserve => {
                self.flag_war_on_event(script, player, event)
            }
            HallScript::RainbowSpringsChateau => self.rainbow_on_event(player, npc, npc_id, event),
            // The other three have no `onEvent`: `Script.onEvent` returns null.
            _ => None,
        };
        self.hall_show_result(script, player, result.as_deref(), Some(npc));
    }

    // ------------------------------------------------------------------ the first click

    /// The `ON_NPC_FIRST_TALK` listeners (and `ON_NPC_QUEST_START` of the start NPCs) of the hall
    /// scripts. `true` when a script took the talk.
    pub(crate) fn hall_script_first_talk(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let Some((script, start_npc)) = first_talk_script(npc_id) else {
            return false;
        };
        if !self.hall_script_loaded(script) {
            return false;
        }
        // `addStartNpc`: `ON_NPC_QUEST_START` makes the NPC the quest NPC of the player.
        if start_npc && let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
        match script {
            HallScript::FortressOfResistance => self.resistance_first_talk(player),
            HallScript::BanditStronghold | HallScript::WildBeastReserve => {
                let result = self.flag_war_first_talk(script, npc, npc_id, player);
                self.hall_show_result(script, player, result.as_deref(), Some(npc));
            }
            HallScript::RainbowSpringsChateau => self.rainbow_first_talk(npc, npc_id, player),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ NPC events

    /// `ON_ATTACKABLE_ATTACK`: called by `Attackable.addDamage`; the listeners are notified
    /// asynchronously, so the script runs one tick later (the NPC's HP is after the hit).
    pub(crate) fn hall_script_on_attack(&mut self, npc: ObjectId, attacker: ObjectId, damage: i32) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let Some(script) = attack_script(npc_id) else {
            return;
        };
        if !self.hall_script_loaded(script) {
            return;
        }
        // `attacker.asPlayer()`: a summon's owner.
        let Some(player) = self.acting_player(attacker) else {
            return;
        };
        self.hall_schedule(HallSiegeTimer::Attacked { npc, player, damage }, 0);
    }

    /// The script's `onAttack`.
    pub(crate) fn hall_script_on_attack_due(&mut self, npc: ObjectId, player: ObjectId, damage: i32) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let Some(script) = attack_script(npc_id) else {
            return;
        };
        match script {
            HallScript::FortressOfResistance => self.resistance_on_attack(player, damage),
            HallScript::DevastatedCastle => self.devastated_on_attack(npc, player, damage),
            HallScript::FortressOfTheDead => self.dead_on_attack(player, damage),
            _ => {}
        }
    }

    /// `ON_ATTACKABLE_KILL`: `Attackable.doDie` notifies the listeners `_onKillDelay` later.
    pub(crate) fn hall_script_killed(&mut self, npc: ObjectId, killer: Option<ObjectId>) {
        let Some((npc_id, spawn)) = self.npcs.get(&npc).map(|n| (n.npc_id, n.spawn)) else {
            return;
        };
        let Some(script) = kill_script(npc_id) else {
            return;
        };
        if !self.hall_script_loaded(script) {
            return;
        }
        let Some(killer) = killer.and_then(|k| self.acting_player(k)) else {
            return;
        };
        self.hall_schedule(
            HallSiegeTimer::Kill {
                hall: ClanHallId(script.hall_id()),
                npc,
                npc_id,
                spawn,
                killer,
            },
            KILL_DELAY_MS,
        );
    }

    /// The script's `onKill` once the delay is over.
    pub(crate) fn hall_script_on_kill(
        &mut self,
        hall: ClanHallId,
        npc: ObjectId,
        npc_id: i32,
        spawn: Option<usize>,
        killer: ObjectId,
    ) {
        let Some(script) = HallScript::of_hall(hall.0) else {
            return;
        };
        match script {
            HallScript::FortressOfResistance => self.resistance_on_kill(npc, spawn),
            HallScript::DevastatedCastle => self.devastated_on_kill(),
            HallScript::FortressOfTheDead => self.dead_on_kill(npc, npc_id),
            HallScript::BanditStronghold | HallScript::WildBeastReserve => {
                self.flag_war_on_kill(script, npc_id);
            }
            HallScript::RainbowSpringsChateau => self.rainbow_on_kill(npc, npc_id, killer),
        }
    }

    /// `ON_NPC_SPAWN`: `Npc.onSpawn` notifies the listeners asynchronously, so the script runs one
    /// tick later, when the players around know the NPC.
    pub(crate) fn hall_script_on_spawn(&mut self, npc: ObjectId) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let Some(script) = spawn_script(npc_id) else {
            return;
        };
        if self.hall_script_loaded(script) {
            self.hall_schedule(HallSiegeTimer::NpcSpawned(npc), 0);
        }
    }

    /// The script's `onSpawn` once the NPC is known to the players around.
    pub(crate) fn hall_script_on_spawn_due(&mut self, npc: ObjectId) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let Some(script) = spawn_script(npc_id) else {
            return;
        };
        match script {
            HallScript::DevastatedCastle => self.devastated_on_spawn(npc, npc_id),
            HallScript::FortressOfTheDead => self.dead_on_spawn(npc, npc_id),
            HallScript::BanditStronghold | HallScript::WildBeastReserve => {
                self.flag_war_on_spawn(script, npc);
            }
            HallScript::RainbowSpringsChateau => self.rainbow_on_spawn(npc, npc_id),
            HallScript::FortressOfResistance => {}
        }
    }

    /// `ON_NPC_SKILL_SEE`: `Creature.callSkill` tells the NPCs within 1000.
    pub(crate) fn hall_script_on_skill_see(
        &mut self,
        npc: ObjectId,
        caster: ObjectId,
        skill_id: i32,
        targets: &[ObjectId],
    ) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        if !rainbow::YETIS.contains(&npc_id) || !self.hall_script_loaded(HallScript::RainbowSpringsChateau) {
            return;
        }
        if let Some(player) = self.acting_player(caster) {
            self.rainbow_on_skill_see(npc, player, skill_id, targets);
        } else {
            debug!("{caster}: a skill seen by a yeti, cast by no player.");
        }
    }

    /// Java `Npc.getConquerableHall()`: the conquerable hall within 10000 of the position.
    pub fn hall_siege_near(&self, x: i32, y: i32) -> Option<ClanHallId> {
        self.nearby_siegable_hall(x, y, 10_000)
    }

    /// The halls' ids that have a script, for the lists.
    pub fn hall_siege_ids(&self) -> Vec<ClanHallId> {
        [
            FORTRESS_RESISTANCE,
            DEVASTATED_CASTLE,
            BANDIT_STRONGHOLD,
            RAINBOW_SPRINGS,
            BEAST_FARM,
            FORTRESS_OF_DEAD,
        ]
        .into_iter()
        .map(ClanHallId)
        .filter(|h| self.hall_has_siege(*h))
        .collect()
    }
}
