//! Rainbow Springs Chateau (clan hall 62): a contest of up to four clans, each in its own arena,
//! who collect the letters of a word from chests, trade them for rainbow items, and use those on
//! a yeti to hurt the others' gourds; the first gourd to fall loses.
// Java: dist/game/data/scripts/conquerablehalls/RainbowSpringsChateau/RainbowSpringsChateau.java
//
// NOTE(java-quirk): the script never sets the status to running, `registerClan` puts the
// *registering* clan in `_acceptedClans` up to four times (so the first clan to register fills
// all the arenas), `Cancel` only deletes the row, `_acceptedClans` is not cleared when a siege
// ends, the yetis are not spawned again for a second siege, and all the `GenerateTask`s but the
// last one run for ever. All of it is ported as it is.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, ExShowScreenMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::engine::{ScriptError, ScriptResult};
use super::load::HallSiegeWrite;
use super::schedule::format_hour_day;
use super::{HallSiegeTimer, RAINBOW_SPRINGS, ScriptState, SiegeStatus};
use crate::clan::model::java_hash_map_order;
use crate::effects::SkillKey;
use crate::residence::{ClanHallId, ResidenceRef, ResidenceWrite};
use crate::target::distance_2d;
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::timer::TimerId;
use crate::world::World;

const WAR_DECREES: i32 = 8034;
const RAINBOW_NECTAR: i32 = 8030;
const RAINBOW_MWATER: i32 = 8031;
const RAINBOW_WATER: i32 = 8032;
const RAINBOW_SULFUR: i32 = 8033;

pub(super) const MESSENGER: i32 = 35604;
pub(super) const CARETAKER: i32 = 35603;
pub(super) const CHEST: i32 = 35593;
pub(super) const ENRAGED_YETI: i32 = 35592;
pub(super) const GOURDS: [i32; 4] = [35588, 35589, 35590, 35591];
pub(super) const YETIS: [i32; 4] = [35596, 35597, 35598, 35599];

const ITEM_A: i32 = 8035;
const ITEM_B: i32 = 8036;
const ITEM_C: i32 = 8037;
const ITEM_D: i32 = 8038;
const ITEM_E: i32 = 8039;
const ITEM_F: i32 = 8040;
const ITEM_G: i32 = 8041;
const ITEM_H: i32 = 8042;
const ITEM_I: i32 = 8043;
const ITEM_K: i32 = 8045;
const ITEM_L: i32 = 8046;
const ITEM_N: i32 = 8047;
const ITEM_O: i32 = 8048;
const ITEM_P: i32 = 8049;
const ITEM_R: i32 = 8050;
const ITEM_S: i32 = 8051;
const ITEM_T: i32 = 8052;
const ITEM_U: i32 = 8053;
const ITEM_W: i32 = 8054;
const ITEM_Y: i32 = 8055;

/// The letters a chest drops, in the order of its chance bands.
const LETTERS: [i32; 20] = [
    ITEM_A, ITEM_B, ITEM_C, ITEM_D, ITEM_E, ITEM_F, ITEM_G, ITEM_H, ITEM_I, ITEM_K, ITEM_L, ITEM_N, ITEM_O,
    ITEM_P, ITEM_R, ITEM_S, ITEM_T, ITEM_U, ITEM_W, ITEM_Y,
];

fn debuff() -> SkillKey {
    SkillKey::new(4991, 1)
}

const ARENAS: [(i32, i32, i32); 4] = [
    (151562, -127080, -2214),
    (153141, -125335, -2214),
    (153892, -127530, -2214),
    (155657, -125752, -2214),
];
// NOTE(java-quirk): the fourth point has 156657 where the arena has 155657.
const YETIS_SPAWN: [(i32, i32, i32); 4] = [
    (151560, -127075, -2221),
    (153129, -125337, -2221),
    (153884, -127534, -2221),
    (156657, -125753, -2221),
];
const CHESTS_SPAWN: [(i32, i32, i32); 4] = [
    (151560, -127075, -2221),
    (153129, -125337, -2221),
    (153884, -127534, -2221),
    (155657, -125753, -2221),
];

/// A word of the yeti: its name, the items `getItem` destroys and the items the player must have.
struct Word {
    name: &'static str,
    destroy: &'static [(i32, i64)],
    needs: &'static [(i32, i64)],
}

const WORD_LIST: [Word; 8] = [
    Word {
        name: "BABYDUCK",
        destroy: &[
            (ITEM_B, 2),
            (ITEM_A, 1),
            (ITEM_Y, 1),
            (ITEM_D, 1),
            (ITEM_U, 1),
            (ITEM_C, 1),
            (ITEM_K, 1),
        ],
        needs: &[
            (ITEM_B, 2),
            (ITEM_A, 1),
            (ITEM_Y, 1),
            (ITEM_D, 1),
            (ITEM_U, 1),
            (ITEM_C, 1),
            (ITEM_K, 1),
        ],
    },
    Word {
        name: "ALBATROS",
        destroy: &[
            (ITEM_A, 2),
            (ITEM_L, 1),
            (ITEM_B, 1),
            (ITEM_T, 1),
            (ITEM_R, 1),
            (ITEM_O, 1),
            (ITEM_S, 1),
        ],
        needs: &[
            (ITEM_A, 2),
            (ITEM_L, 1),
            (ITEM_B, 1),
            (ITEM_T, 1),
            (ITEM_R, 1),
            (ITEM_O, 1),
            (ITEM_S, 1),
        ],
    },
    Word {
        name: "PELICAN",
        destroy: &[
            (ITEM_P, 1),
            (ITEM_E, 1),
            (ITEM_L, 1),
            (ITEM_I, 1),
            (ITEM_C, 1),
            (ITEM_A, 1),
            (ITEM_N, 1),
        ],
        needs: &[
            (ITEM_P, 1),
            (ITEM_E, 1),
            (ITEM_L, 1),
            (ITEM_I, 1),
            (ITEM_C, 1),
            (ITEM_A, 1),
            (ITEM_N, 1),
        ],
    },
    Word {
        name: "KINGFISHER",
        destroy: &[
            (ITEM_K, 1),
            (ITEM_I, 1),
            (ITEM_N, 1),
            (ITEM_G, 1),
            (ITEM_F, 1),
            (ITEM_I, 1),
            (ITEM_S, 1),
            (ITEM_H, 1),
            (ITEM_E, 1),
            (ITEM_R, 1),
        ],
        needs: &[
            (ITEM_K, 1),
            (ITEM_I, 2),
            (ITEM_N, 1),
            (ITEM_G, 1),
            (ITEM_F, 1),
            (ITEM_S, 1),
            (ITEM_H, 1),
            (ITEM_E, 1),
            (ITEM_R, 1),
        ],
    },
    Word {
        name: "CYGNUS",
        destroy: &[
            (ITEM_C, 1),
            (ITEM_Y, 1),
            (ITEM_G, 1),
            (ITEM_N, 1),
            (ITEM_U, 1),
            (ITEM_S, 1),
        ],
        needs: &[
            (ITEM_C, 1),
            (ITEM_Y, 1),
            (ITEM_G, 1),
            (ITEM_N, 1),
            (ITEM_U, 1),
            (ITEM_S, 1),
        ],
    },
    // NOTE(java-quirk): the word has no O in the list of items to destroy, the check asks for one.
    Word {
        name: "TRITON",
        destroy: &[(ITEM_T, 2), (ITEM_R, 1), (ITEM_I, 1), (ITEM_N, 1)],
        needs: &[(ITEM_T, 2), (ITEM_R, 1), (ITEM_I, 1), (ITEM_O, 1), (ITEM_N, 1)],
    },
    Word {
        name: "RAINBOW",
        destroy: &[
            (ITEM_R, 1),
            (ITEM_A, 1),
            (ITEM_I, 1),
            (ITEM_N, 1),
            (ITEM_B, 1),
            (ITEM_O, 1),
            (ITEM_W, 1),
        ],
        needs: &[
            (ITEM_R, 1),
            (ITEM_A, 1),
            (ITEM_I, 1),
            (ITEM_N, 1),
            (ITEM_B, 1),
            (ITEM_O, 1),
            (ITEM_W, 1),
        ],
    },
    Word {
        name: "SPRING",
        destroy: &[
            (ITEM_S, 1),
            (ITEM_P, 1),
            (ITEM_R, 1),
            (ITEM_I, 1),
            (ITEM_N, 1),
            (ITEM_G, 1),
        ],
        needs: &[
            (ITEM_S, 1),
            (ITEM_P, 1),
            (ITEM_R, 1),
            (ITEM_I, 1),
            (ITEM_N, 1),
            (ITEM_G, 1),
        ],
    },
];

/// What the script keeps.
#[derive(Debug, Default)]
pub(crate) struct Rainbow {
    /// `_warDecreesCount`: clan id to the war decrees it paid (a static `HashMap`).
    pub(crate) war_decrees: Vec<(i32, i64)>,
    /// `_acceptedClans`: the clan of each arena (`None` where `ClanTable` did not know it).
    pub(crate) accepted: Vec<Option<i32>>,
    /// `_playersOnArena`.
    pub(crate) players_on_arena: Vec<i32>,
    /// `chests`.
    pub(crate) chests: Vec<ObjectId>,
    /// `_generated`: the index of the word, -1 for none.
    pub(crate) generated: i32,
    /// `_winner`.
    pub(crate) winner: Option<i32>,
    /// `_gourds`: spawn and NPC of each arena's gourd.
    pub(crate) gourds: [Option<(usize, ObjectId)>; 4],
    /// `_yetis`.
    pub(crate) yetis: [Option<(usize, ObjectId)>; 4],
    /// The pending `GenerateTask` of each yeti.
    pub(crate) yeti_timers: [Option<TimerId>; 4],
    /// `_task`: the yeti whose task the script remembers (the last one spawned).
    pub(crate) task_yeti: Option<usize>,
    /// `_chesttask`.
    pub(crate) chest_task: Option<TimerId>,
    /// `arenaChestsCnt`.
    pub(crate) arena_chests: [i32; 4],
}

impl Rainbow {
    pub(crate) fn new() -> Self {
        Self {
            generated: -1,
            ..Self::default()
        }
    }

    fn keys(&self) -> Vec<i32> {
        let keys: Vec<i32> = self.war_decrees.iter().map(|(c, _)| *c).collect();
        java_hash_map_order(&keys).into_iter().map(|i| keys[i]).collect()
    }

    fn index_of(&self, clan: Option<i32>) -> Option<usize> {
        self.accepted.iter().position(|c| *c == clan)
    }
}

fn hall() -> ClanHallId {
    ClanHallId(RAINBOW_SPRINGS)
}

const PAGES: &str = "data/scripts/conquerablehalls/RainbowSpringsChateau";

impl World {
    fn rainbow(&self) -> Option<&Rainbow> {
        match self.hall_siege(hall()).map(|s| &s.state) {
            Some(ScriptState::Rainbow(s)) => Some(s),
            _ => None,
        }
    }

    fn rainbow_mut(&mut self) -> Option<&mut Rainbow> {
        match self.hall_siege_mut(hall()).map(|s| &mut s.state) {
            Some(ScriptState::Rainbow(s)) => Some(s),
            _ => None,
        }
    }

    /// The clans of the arenas (for the tests).
    pub fn rainbow_accepted_clans(&self) -> Vec<Option<i32>> {
        self.rainbow().map(|s| s.accepted.clone()).unwrap_or_default()
    }

    /// The war decrees each clan paid (for the tests).
    pub fn rainbow_war_decrees(&self) -> Vec<(i32, i64)> {
        self.rainbow().map(|s| s.war_decrees.clone()).unwrap_or_default()
    }

    /// The word the yetis chose (-1 for none).
    pub fn rainbow_generated_word(&self) -> i32 {
        self.rainbow().map_or(-1, |s| s.generated)
    }

    fn rainbow_write(&mut self, write: HallSiegeWrite) -> bool {
        self.siege_write(ResidenceWrite::HallSiege(write))
    }

    fn page(&self, player: ObjectId, name: &str) -> String {
        self.hall_page_text(player, &format!("{PAGES}/{name}"))
    }

    /// Java `Script.addSpawn(npcId, x, y, z, heading, false, 0, false)`: a spawn that does not
    /// respawn.
    fn rainbow_add_spawn(
        &mut self,
        npc_id: i32,
        pos: (i32, i32, i32),
        heading: i32,
    ) -> Option<(usize, ObjectId)> {
        if pos.0 == 0 && pos.1 == 0 {
            return None;
        }
        let spawn = self.fort_new_spawn(npc_id, pos, heading, 0);
        self.stop_respawn(spawn);
        let npc = self.fort_do_spawn(spawn)?;
        Some((spawn, npc))
    }

    // ------------------------------------------------------------------ registerClan

    /// Java `registerClan(clan, count, register)`.
    fn rainbow_register_clan(&mut self, clan_id: i32, count: i64, register: bool) -> ScriptResult {
        if !register {
            self.rainbow_write(HallSiegeWrite::RainbowRemove { clan_id });
            return Ok(());
        }
        self.hall_add_attacker(hall(), clan_id);
        for _ in 0..4 {
            let mut counter = 0;
            let mut fight_clan = None;
            let keys = self.rainbow().map(Rainbow::keys).unwrap_or_default();
            for (position, key) in keys.iter().enumerate() {
                let acting_ok = self.clan(*key).is_some_and(|c| c.dissolving_expiry_time <= 0);
                if !acting_ok {
                    if let Some(state) = self.rainbow_mut() {
                        state.war_decrees.retain(|(c, _)| c != key);
                    }
                    // NOTE(java-quirk): removing from the map while its keys are walked throws on
                    // the next key.
                    if position + 1 < keys.len() {
                        return Err(ScriptError("java.util.ConcurrentModificationException"));
                    }
                    continue;
                }
                let counts = self
                    .rainbow()
                    .and_then(|s| s.war_decrees.iter().find(|(c, _)| c == key))
                    .map_or(0, |(_, n)| *n);
                if counts > counter {
                    counter = counts;
                    fight_clan = Some(*key);
                }
            }
            if let Some(state) = self.rainbow_mut()
                && fight_clan.is_some()
                && state.accepted.len() < 4
            {
                // NOTE(java-quirk): adds the registering clan, not `fightclan`.
                state.accepted.push(Some(clan_id));
            }
        }
        self.rainbow_write(HallSiegeWrite::RainbowAdd { clan_id, count });
        Ok(())
    }

    // ------------------------------------------------------------------ onEvent

    /// Java `onEvent(event, npc, player)`: the text for `showResult`.
    pub(super) fn rainbow_on_event(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        npc_id: i32,
        event: &str,
    ) -> Option<String> {
        let clan = self.clan_of(player).map(|c| {
            (
                c.id,
                c.level,
                c.members_count(),
                c.castle_id,
                c.fort_id,
                c.hideout_id,
            )
        });
        let leader = self.is_clan_leader(player);
        let mut htmltext = Some(event.to_owned());
        let owns_residence = clan.is_some_and(|c| c.3 > 0 || c.4 > 0 || c.5 > 0);
        let clan_id = clan.map(|c| c.0);
        if npc_id == MESSENGER {
            match event {
                "Register" => {
                    let cid = clan_id.unwrap_or(0);
                    htmltext = Some(if !leader {
                        "35604-07.htm".to_owned()
                    } else if owns_residence {
                        "35604-09.htm".to_owned()
                    } else if !self.hall_is_registering(hall()) {
                        "35604-11.htm".to_owned()
                    } else if self
                        .rainbow()
                        .is_some_and(|s| s.war_decrees.iter().any(|(c, _)| *c == cid))
                    {
                        "35604-10.htm".to_owned()
                    } else if self.hall_siege(hall()).is_some_and(|s| s.attackers.len() >= 4) {
                        "35604-18.htm".to_owned()
                    } else if clan.is_some_and(|c| c.1 < 3 || c.2 < 5) {
                        "35604-08.htm".to_owned()
                    } else {
                        match self
                            .inventory_item_by_item_id(player, WAR_DECREES)
                            .map(|i| i.count)
                        {
                            None => "35604-05.htm".to_owned(),
                            Some(count) => {
                                if let Some(state) = self.rainbow_mut() {
                                    state.war_decrees.push((cid, count));
                                }
                                self.destroy_item_by_item_id(player, WAR_DECREES, -1, true);
                                match self.rainbow_register_clan(cid, count, true) {
                                    Ok(()) => "35604-06.htm".to_owned(),
                                    Err(e) => {
                                        warn!("RainbowSpringsChateau: registerClan failed: {}", e.0);
                                        return None;
                                    }
                                }
                            }
                        }
                    });
                }
                "Cancel" => {
                    let cid = clan_id.unwrap_or(0);
                    htmltext = Some(if !leader {
                        "35604-08.htm".to_owned()
                    } else if !self
                        .rainbow()
                        .is_some_and(|s| s.war_decrees.iter().any(|(c, _)| *c == cid))
                    {
                        "35604-12.htm".to_owned()
                    } else if !self.hall_is_registering(hall()) {
                        "35604-13.htm".to_owned()
                    } else {
                        let _ = self.rainbow_register_clan(cid, 0, false);
                        "35604-17.htm".to_owned()
                    });
                }
                "Unregister" => {
                    let cid = clan_id.unwrap_or(0);
                    if !leader {
                        htmltext = Some("35604-07.htm".to_owned());
                    } else if self.hall_is_registering(hall()) {
                        let paid = self
                            .rainbow()
                            .and_then(|s| s.war_decrees.iter().find(|(c, _)| *c == cid))
                            .map(|(_, n)| *n);
                        if let Some(count) = paid {
                            self.add_item(player, WAR_DECREES, count / 2, true);
                            if let Some(state) = self.rainbow_mut() {
                                state.war_decrees.retain(|(c, _)| *c != cid);
                            }
                            htmltext = Some("35604-14.htm".to_owned());
                        } else {
                            htmltext = Some("35604-16.htm".to_owned());
                        }
                    } else if self.hall_is_waiting_battle(hall()) {
                        if let Some(state) = self.rainbow_mut()
                            && let Some(position) = state.accepted.iter().position(|c| *c == clan_id)
                        {
                            state.accepted.remove(position);
                        }
                        htmltext = Some("35604-16.htm".to_owned());
                    }
                }
                _ => {}
            }
        } else if npc_id == CARETAKER && event == "GoToArena" {
            return self.rainbow_go_to_arena(player, clan);
        }
        if event.starts_with("getItem") {
            self.rainbow_get_item(player, npc);
            return None;
        } else if event.starts_with("seeItem") {
            let generated = self.rainbow().map_or(-1, |s| s.generated);
            let word = usize::try_from(generated)
                .ok()
                .and_then(|i| WORD_LIST.get(i))
                .map_or_else(
                    // NOTE(java-quirk): the string id is written with `toString()`, "NS[id:name]".
                    || {
                        format!(
                            "<fstring>NS[{}:{}]</fstring>",
                            NpcStringId::Undecided.id(),
                            NpcStringId::Undecided.name()
                        )
                    },
                    |w| w.name.to_owned(),
                );
            let html = self.page(player, "35596-05.htm").replace("%word%", &word);
            self.rainbow_send(player, npc, &html);
            return None;
        }
        htmltext
    }

    fn rainbow_send(&mut self, player: ObjectId, npc: ObjectId, html: &str) {
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, html) {
            self.send_html(player, &message);
        }
    }

    /// The `GoToArena` event.
    fn rainbow_go_to_arena(
        &mut self,
        player: ObjectId,
        clan: Option<(i32, i32, usize, i32, i32, i32)>,
    ) -> Option<String> {
        let party = self.party_of(player);
        let Some((clan_id, level, _, castle, fort, hideout)) = clan else {
            return Some("35603-07.htm".to_owned());
        };
        if !self.is_clan_leader(player) {
            return Some("35603-02.htm".to_owned());
        }
        let Some(party) = party else {
            return Some("35603-03.htm".to_owned());
        };
        if self.party_leader(party) != Some(player) {
            return Some("35603-04.htm".to_owned());
        }
        // NOTE(java-quirk): `player.getId()` is the class id, so this compares classes; and the
        // pages 05, 06, 08, 09 and 10 are set and never shown, Java returns null after them.
        let class_of = |world: &World, p: ObjectId| world.players.get(&p).map_or(0, |pl| pl.data.class_id);
        let _non_clan_member = self
            .party_members(party)
            .into_iter()
            .any(|m| class_of(self, m) != class_of(self, player));
        let min_level = self.clanhall.siege.settings.min_clan_level;
        if castle > 0 || fort > 0 || hideout > 0 || level < min_level {
            return None;
        }
        let index = self.rainbow().and_then(|s| s.index_of(Some(clan_id)));
        if let Some(index) = index {
            self.rainbow_port_to_arena(player, party, i32::try_from(index).unwrap_or(-1));
        }
        None
    }

    /// Java `portToArena(leader, arena)`.
    fn rainbow_port_to_arena(&mut self, leader: ObjectId, party: crate::party::PartyId, arena: i32) {
        let Some(&(x, y, z)) = usize::try_from(arena).ok().and_then(|a| ARENAS.get(a)) else {
            warn!("RainbowSpringsChateau: Wrong arena id passed: {arena}");
            return;
        };
        let _ = leader;
        let offset = self.settings.teleport.max_offset_on_teleport;
        for member in self.party_members(party) {
            self.stop_all_effects(member);
            if let Some(summon) = self.summon_of(member) {
                self.unsummon(summon, Some(member));
            }
            if let Some(state) = self.rainbow_mut() {
                state.players_on_arena.push(member.0);
            }
            self.tele_to_location(member, TeleportTarget::at(x, y, z).with_random_offset(offset));
        }
    }

    /// The `getItem` event: a player who has the letters of the word gets a rainbow item.
    fn rainbow_get_item(&mut self, player: ObjectId, npc: ObjectId) {
        let generated = self.rainbow().map_or(-1, |s| s.generated);
        let Some(word) = usize::try_from(generated).ok().and_then(|i| WORD_LIST.get(i)) else {
            return;
        };
        let has = word
            .needs
            .iter()
            .all(|(item, count)| self.inventory_item_count(player, *item) >= *count);
        if has {
            for (item, count) in word.destroy {
                self.destroy_item_by_item_id(player, *item, *count, true);
            }
            let rnd = self.rng().below(100);
            let reward = if generated <= 5 {
                match rnd {
                    0..=69 => RAINBOW_NECTAR,
                    70..=79 => RAINBOW_MWATER,
                    80..=89 => RAINBOW_WATER,
                    _ => RAINBOW_SULFUR,
                }
            } else {
                match rnd {
                    0..=9 => RAINBOW_NECTAR,
                    10..=39 => RAINBOW_MWATER,
                    40..=69 => RAINBOW_WATER,
                    _ => RAINBOW_SULFUR,
                }
            };
            self.add_item(player, reward, 1, true);
        }
        let html = self.page(player, if has { "35596-04.htm" } else { "35596-02.htm" });
        self.rainbow_send(player, npc, &html);
    }

    // ------------------------------------------------------------------ onFirstTalk

    /// Java `onFirstTalk(npc, player)`.
    pub(super) fn rainbow_first_talk(&mut self, npc: ObjectId, npc_id: i32, player: ObjectId) {
        let owner = self.clan_hall(hall()).map_or(0, |h| h.owner_id);
        let clan = self.clan_of(player).map(|c| c.id);
        let html = if npc_id == MESSENGER {
            let main = if owner > 0 { "35604-01.htm" } else { "35604-00.htm" };
            let next = self.hall_next_siege_time(hall());
            let mut html = self
                .page(player, main)
                .replace("%nextSiege%", &format_hour_day(next, self.siege_offset_at(next)));
            if owner > 0 {
                // `ClanTable.getClan(ownerId).getName()` throws for a missing clan.
                let Some(name) = self.clan(owner).map(|c| c.name.clone()) else {
                    return;
                };
                html = html.replace("%owner%", &name);
            }
            Some(html)
        } else if npc_id == CARETAKER {
            let main = if self.hall_is_in_siege(hall()) || !self.hall_is_waiting_battle(hall()) {
                "35603-00.htm"
            } else {
                "35603-01.htm"
            };
            Some(self.page(player, main))
        } else if let Some(yeti) = YETIS.iter().position(|y| *y == npc_id) {
            let index = self.rainbow().and_then(|s| s.index_of(clan));
            let main = match index {
                Some(index) if index == yeti => {
                    if self.is_clan_leader(player) {
                        "35596-01.htm"
                    } else {
                        "35596-00.htm"
                    }
                }
                _ => "35596-06.htm",
            };
            Some(self.page(player, main))
        } else {
            None
        };
        if let Some(html) = html {
            self.rainbow_send(player, npc, &html);
        }
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
    }

    // ------------------------------------------------------------------ skills, kills, spawns

    /// Java `onSkillSee(npc, caster, skill, targets, isSummon)`.
    pub(super) fn rainbow_on_skill_see(
        &mut self,
        npc: ObjectId,
        caster: ObjectId,
        skill_id: i32,
        targets: &[ObjectId],
    ) {
        if !targets.contains(&npc) {
            return;
        }
        let clan = self.clan_of(caster).map(|c| c.id);
        let Some(index) = clan.and_then(|c| self.rainbow().and_then(|s| s.index_of(Some(c)))) else {
            return;
        };
        let close = match (self.position_of(npc), self.position_of(caster)) {
            (Some(a), Some(b)) => distance_2d(a, b) < 60.0,
            _ => false,
        };
        if !close {
            return;
        }
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        match skill_id {
            2240 => {
                if self.rng().below(100) < 10
                    && let Some((x, y, z)) = self.position_of(caster)
                {
                    self.rainbow_add_spawn(ENRAGED_YETI, (x + 10, y + 10, z), 0);
                }
                self.rainbow_reduce_gourd_hp(index, caster);
            }
            2241 => {
                let war = self.rainbow_rnd_ex(size, index);
                self.rainbow_increase_gourd_hp(war);
            }
            2242 => {
                let war = self.rainbow_rnd_ex(size, index);
                self.rainbow_move_gourds(war);
            }
            2243 => {
                let war = self.rainbow_rnd_ex(size, index);
                self.rainbow_cast_debuffs_on_enemies(caster, war);
            }
            _ => {}
        }
    }

    /// Java `rndEx(size, ex)`: a random arena that is not `ex`, tried 127 times.
    fn rainbow_rnd_ex(&mut self, size: usize, ex: usize) -> usize {
        let bound = i32::try_from(size).unwrap_or(i32::MAX);
        let mut rnd = 0;
        for _ in 0..127 {
            rnd = usize::try_from(self.rng().below(bound)).unwrap_or(0);
            if rnd != ex {
                break;
            }
        }
        rnd
    }

    fn rainbow_gourd(&self, index: usize) -> Option<ObjectId> {
        let (_, npc) = self.rainbow()?.gourds.get(index).copied().flatten()?;
        self.npcs.contains_key(&npc).then_some(npc)
    }

    fn rainbow_reduce_gourd_hp(&mut self, index: usize, player: ObjectId) {
        if let Some(gourd) = self.rainbow_gourd(index) {
            self.reduce_current_hp(gourd, 1000.0, Some(player));
        }
    }

    fn rainbow_increase_gourd_hp(&mut self, index: usize) {
        if let Some(gourd) = self.rainbow_gourd(index) {
            let hp = self.npcs.get(&gourd).map_or(0.0, |n| n.core.cur_hp);
            self.set_current_hp(gourd, hp + 1000.0, true);
        }
    }

    /// Java `moveGourds(index)`: the array is walked in place (`tempArray` is the same array).
    fn rainbow_move_gourds(&mut self, index: usize) {
        for i in 0..index {
            let Some(state) = self.rainbow() else {
                return;
            };
            let (Some(old), Some(cur)) = (
                state.gourds.get(index - 1 - i).copied().flatten(),
                state.gourds.get(i).copied().flatten(),
            ) else {
                return;
            };
            if let Some(state) = self.rainbow_mut() {
                state.gourds[index - 1 - i] = Some(cur);
            }
            if let Some((x, y, z)) = self.position_of(old.1) {
                self.tele_to_location(
                    cur.1,
                    TeleportTarget::at(x, y, z)
                        .with_random_offset(self.settings.teleport.max_offset_on_teleport),
                );
            }
        }
    }

    /// Java `castDebuffsOnEnemies(player, myArena)`.
    fn rainbow_cast_debuffs_on_enemies(&mut self, player: ObjectId, my_arena: usize) {
        let clan = self.clan_of(player).map(|c| c.id);
        let Some(index) = self.rainbow().and_then(|s| s.index_of(clan)) else {
            return;
        };
        if !self
            .rainbow()
            .is_some_and(|s| s.players_on_arena.contains(&player.0))
        {
            return;
        }
        let Some(party) = self.party_of(player) else {
            return;
        };
        for member in self.party_members(party) {
            if index != my_arena {
                continue;
            }
            self.apply_effects(member, member, debuff());
        }
    }

    /// Java `onKill(npc, killer, isSummon)`.
    pub(super) fn rainbow_on_kill(&mut self, npc: ObjectId, npc_id: i32, killer: ObjectId) {
        let clan = self.clan_of(killer).map(|c| c.id);
        let Some(clan_id) = clan else {
            return;
        };
        let Some(index) = self.rainbow().and_then(|s| s.index_of(Some(clan_id))) else {
            return;
        };
        if npc_id == CHEST {
            // `chestDie`: every arena has a chest less.
            if let Some(state) = self.rainbow_mut() {
                let count = state.accepted.len().min(4);
                for cnt in state.arena_chests.iter_mut().take(count) {
                    *cnt -= 1;
                }
                state.chests.retain(|c| *c != npc);
            }
            let chance = self.rng().below(100);
            let letter = if chance <= 5 {
                0
            } else if chance <= 95 {
                usize::try_from((chance - 1) / 5).unwrap_or(0)
            } else {
                19
            };
            self.npc_drop_item(npc, Some(killer), LETTERS[letter], 1);
        }
        if GOURDS.get(index) == Some(&npc_id) {
            if let Some(state) = self.rainbow_mut() {
                state.winner = Some(clan_id);
            }
            if let Some(siege) = self.hall_siege_mut(hall()) {
                siege.mission_accomplished = true;
            }
            self.hall_cancel_siege_task(hall());
            self.hall_end_siege(hall());
            self.hall_schedule(HallSiegeTimer::RainbowArenaExit(hall()), 120_000);
        }
    }

    /// The lambda of `onKill` (120 s): the arena's players go to the town.
    pub(crate) fn rainbow_arena_exit_due(&mut self, _hall: ClanHallId) {
        let players = self
            .rainbow_mut()
            .map(|s| std::mem::take(&mut s.players_on_arena))
            .unwrap_or_default();
        for id in players {
            let player = ObjectId(id);
            if self.players.contains_key(&player) {
                self.tele_to_location_where(player, TeleportWhereType::Town);
            }
        }
    }

    /// Java `onSpawn(npc)`.
    pub(super) fn rainbow_on_spawn(&mut self, npc: ObjectId, npc_id: i32) {
        if npc_id == ENRAGED_YETI {
            self.hall_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::OoohWhoPouredNectarOnMyHeadWhileIWasSleeping,
            );
        }
        if GOURDS.contains(&npc_id) {
            self.set_immobilized(npc, true);
            // `disableCoreAI(true)`: the gourd does nothing by itself.
            self.stop_ai_task(npc);
        }
    }

    // ------------------------------------------------------------------ the tasks

    /// Java `spawnGourds()`.
    fn rainbow_spawn_gourds(&mut self) {
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        if size < 2 {
            return;
        }
        for i in 0..size {
            let (Some(gourd), Some(&(x, y, z))) = (GOURDS.get(i), ARENAS.get(i)) else {
                warn!("RainbowSpringsChateau: Problem with spawnGourds: index {i}");
                break;
            };
            let made = self.rainbow_add_spawn(*gourd, (x + 150, y + 150, z), 1);
            if let Some(state) = self.rainbow_mut() {
                state.gourds[i] = made;
            }
        }
        let timer = self.hall_schedule(HallSiegeTimer::RainbowChests(hall()), 5000);
        if let Some(state) = self.rainbow_mut() {
            state.chest_task = Some(timer);
        }
    }

    /// Java `spawnYetis()`.
    fn rainbow_spawn_yetis(&mut self) {
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        for i in 0..size {
            let (Some(yeti), Some(&pos)) = (YETIS.get(i), YETIS_SPAWN.get(i)) else {
                warn!("RainbowSpringsChateau: Problem with spawnYetis: index {i}");
                break;
            };
            // NOTE(java-quirk): `_yetis[i]` is never emptied, a second siege spawns no yeti.
            if self.rainbow().is_some_and(|s| s.yetis[i].is_some()) {
                continue;
            }
            let made = self.rainbow_add_spawn(*yeti, pos, 0);
            if let Some((_, npc)) = made {
                self.set_heading(npc, 1);
            }
            let timer = self.hall_schedule(
                HallSiegeTimer::RainbowGenerate {
                    hall: hall(),
                    yeti: i,
                },
                10_000,
            );
            if let Some(state) = self.rainbow_mut() {
                state.yetis[i] = made;
                state.yeti_timers[i] = Some(timer);
                state.task_yeti = Some(i);
            }
        }
    }

    /// Java `GenerateTask.run()`, a fixed rate of 5 minutes.
    pub(crate) fn rainbow_generate_due(&mut self, _hall: ClanHallId, yeti: usize) {
        let timer = self.hall_schedule(HallSiegeTimer::RainbowGenerate { hall: hall(), yeti }, 300_000);
        let npc = {
            let Some(state) = self.rainbow_mut() else {
                self.timers.cancel(timer);
                return;
            };
            state.yeti_timers[yeti] = Some(timer);
            state.yetis[yeti].map(|(_, npc)| npc)
        };
        let word = self.rng().below(i32::try_from(WORD_LIST.len()).unwrap_or(1));
        if let Some(state) = self.rainbow_mut() {
            state.generated = word;
        }
        let Some(npc) = npc else {
            return;
        };
        let Some(word) = usize::try_from(word).ok().and_then(|w| WORD_LIST.get(w)) else {
            return;
        };
        let message = ExShowScreenMessage::text(word.name, 5000);
        let Some(position) = self.position_of(npc) else {
            return;
        };
        let region = self
            .zoning
            .map_regions
            .as_ref()
            .map_or(0, |r| r.map_region_loc_id(position.0, position.1));
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable();
        for player in ids {
            let Some((px, py, _)) = self.position_of(player) else {
                continue;
            };
            let player_region = self
                .zoning
                .map_regions
                .as_ref()
                .map_or(0, |r| r.map_region_loc_id(px, py));
            if player_region == region && self.check_if_in_range(750, npc, player, false) {
                self.send(player, &message);
            }
        }
    }

    /// Java `ChestsSpawn.run()`, every 5 seconds.
    pub(crate) fn rainbow_chests_due(&mut self, _hall: ClanHallId) {
        let timer = self.hall_schedule(HallSiegeTimer::RainbowChests(hall()), 5000);
        match self.rainbow_mut() {
            Some(state) => state.chest_task = Some(timer),
            None => {
                self.timers.cancel(timer);
                return;
            }
        }
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        for (i, &(cx, cy, cz)) in CHESTS_SPAWN.iter().enumerate().take(size) {
            for _ in 0..4 {
                if self.rainbow().is_some_and(|s| s.arena_chests[i] < 4) {
                    let x = cx + self.rng().get(-400, 400);
                    let y = cy + self.rng().get(-400, 400);
                    let chest = self.rainbow_add_spawn(CHEST, (x, y, cz), 0);
                    if let Some(state) = self.rainbow_mut() {
                        if let Some((_, npc)) = chest {
                            state.chests.push(npc);
                        }
                        state.arena_chests[i] += 1;
                    }
                }
            }
        }
    }

    fn rainbow_unspawn_yetis(&mut self) {
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        for i in 0..size.min(4) {
            let yeti = self.rainbow().and_then(|s| s.yetis[i]);
            if let Some((_, npc)) = yeti {
                self.delete_npc(npc);
            }
            // `if (_task != null) { _task.cancel(false); _task = null; }`
            let task = self.rainbow_mut().and_then(|s| {
                let yeti = s.task_yeti.take()?;
                s.yeti_timers[yeti].take()
            });
            if let Some(task) = task {
                self.timers.cancel(task);
            }
        }
    }

    fn rainbow_unspawn_gourds(&mut self) {
        let size = self.rainbow().map_or(0, |s| s.accepted.len());
        for i in 0..size.min(4) {
            if let Some(gourd) = self.rainbow_gourd(i) {
                self.delete_npc(gourd);
            }
        }
    }

    fn rainbow_unspawn_chests(&mut self) {
        let chests = self.rainbow().map(|s| s.chests.clone()).unwrap_or_default();
        for chest in chests {
            self.delete_npc(chest);
            let task = self.rainbow_mut().and_then(|s| s.chest_task.take());
            if let Some(task) = task {
                self.timers.cancel(task);
            }
        }
    }
}

// ---------------------------------------------------------------------------- the engine hooks

/// Java `prepareOwner()`.
pub(super) fn prepare_owner(world: &mut World, hall_id: ClanHallId) -> ScriptResult {
    let owner = world.clan_hall(hall_id).map_or(0, |h| h.owner_id);
    if owner > 0 {
        if world.clan(owner).is_none() {
            return Err(ScriptError("java.lang.NullPointerException"));
        }
        world.rainbow_register_clan(owner, 10_000, true)?;
    }
    world.residence_banish_foreigners(ResidenceRef::ClanHall(hall_id));
    world.hall_broadcast_registration_ended(hall_id);
    world.hall_set_status(hall_id, SiegeStatus::WaitingBattle);
    world.hall_set_task(hall_id, HallSiegeTimer::SiegeStarts(hall_id), 3_600_000);
    Ok(())
}

/// Java `startSiege()`.
pub(super) fn start_siege(world: &mut World, hall_id: ClanHallId) -> ScriptResult {
    let size = world.rainbow().map_or(0, |s| s.accepted.len());
    if size < 2 {
        world.hall_on_siege_ends(hall_id)?;
        if let Some(state) = world.rainbow_mut() {
            state.accepted.clear();
        }
        world.hall_update_next_siege(hall_id);
        world.hall_broadcast_lack_of_interest(hall_id);
        return Ok(());
    }
    world.rainbow_spawn_gourds();
    world.rainbow_spawn_yetis();
    Ok(())
}

/// Java `onSiegeEnds()`.
pub(super) fn on_siege_ends(world: &mut World, _hall: ClanHallId) {
    world.rainbow_unspawn_gourds();
    world.rainbow_unspawn_yetis();
    world.rainbow_unspawn_chests();
    world.rainbow_write(HallSiegeWrite::RainbowClear);
}

/// The state at startup: `loadAttackers` and the constructor.
pub(super) fn new_state(world: &World, rows: &[(i32, i64)]) -> Rainbow {
    let mut state = Rainbow::new();
    for (clan_id, count) in rows {
        match state.war_decrees.iter_mut().find(|(c, _)| c == clan_id) {
            Some((_, n)) => *n = *count,
            None => state.war_decrees.push((*clan_id, *count)),
        }
        // NOTE(java-quirk): every row adds *every* clan of the map again.
        for clan in state.keys() {
            state
                .accepted
                .push(Some(clan).filter(|c| world.clan(*c).is_some()));
        }
    }
    let owner = world.clan_hall(hall()).map_or(0, |h| h.owner_id);
    state.winner = Some(owner).filter(|o| world.clan(*o).is_some());
    state
}

/// D-50: the table as the database has it now; a registration this world holds without a row
/// stays.
pub(super) fn apply_rows(world: &mut World, rows: &[(i32, i64)]) {
    if let Some(state) = world.rainbow_mut() {
        for (clan_id, count) in rows {
            match state.war_decrees.iter_mut().find(|(c, _)| c == clan_id) {
                Some((_, n)) => *n = *count,
                None => state.war_decrees.push((*clan_id, *count)),
            }
        }
    }
}
