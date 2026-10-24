//! Bandit Stronghold (clan hall 35) and Wild Beast Reserve (clan hall 63): the flag war. Up to
//! five clans register at the messenger, each gets a flag and picks a warrior; the last clan whose
//! warrior stands wins the hall. The two Java scripts differ in their constants and the numbers
//! of their pages only.
// Java: dist/game/data/scripts/conquerablehalls/BanditStronghold/BanditStronghold.java
// Java: dist/game/data/scripts/conquerablehalls/WildBeastReserve/WildBeastReserve.java
//
// NOTE(java-quirk): `startSiege()` of the script calls itself again five minutes later instead of
// `super.startSiege()`: the doors open and close and the owner's members are moved to their
// spawn point every five minutes, but the battle never starts. `_firstPhase` is never set to
// true, so the first phase code of `onKill` and `doSpawns` is dead code. Everything is ported as
// it is.

use l2_data::Location;
use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::engine::{ScriptError, ScriptResult};
use super::load::{FlagWarRecord, HallSiegeWrite};
use super::schedule::java_date_string;
use super::{BANDIT_STRONGHOLD, BEAST_FARM, HallScript, HallSiegeTimer, ScriptState, SiegeStatus};
use crate::clan::model::java_hash_map_order;
use crate::residence::{ClanHallId, ResidenceRef, ResidenceWrite};
use crate::teleport::TeleportWhereType;
use crate::world::World;

/// The constants of one of the two scripts.
struct Consts {
    hall: i32,
    script: HallScript,
    /// The numbers of the pages after `prefix-`: `shifted` ones are one higher in Wild Beast
    /// Reserve.
    prefix: &'static str,
    shift: i32,
    royal_flag: i32,
    allies: [i32; 5],
    teleport_1: i32,
    messenger: i32,
    outer_doors: [i32; 2],
    inner_doors: [i32; 2],
    flag_coords: [(i32, i32, i32, i32); 7],
    center: (i32, i32, i32),
    quest_reward: i32,
    /// The item `CheckQuest` asks for and how many.
    quest_item: (i32, i64),
    /// `for (int c = i; c < N; c++)` and the text of an empty place.
    empty_places: i32,
    empty_text: &'static str,
    /// The pages of the teleport NPCs.
    tele_prefix: &'static str,
}

const BANDIT: Consts = Consts {
    hall: BANDIT_STRONGHOLD,
    script: HallScript::BanditStronghold,
    prefix: "35437",
    shift: 0,
    royal_flag: 35422,
    allies: [35428, 35429, 35430, 35431, 35432],
    teleport_1: 35560,
    messenger: 35437,
    outer_doors: [22170001, 22170002],
    inner_doors: [22170003, 22170004],
    flag_coords: [
        (83699, -17468, -1774, 19048),
        (82053, -17060, -1784, 5432),
        (82142, -15528, -1799, 58792),
        (83544, -15266, -1770, 44976),
        (84609, -16041, -1769, 35816),
        (81981, -15708, -1858, 60392),
        (84375, -17060, -1860, 27712),
    ],
    center: (82882, -16280, -1894),
    quest_reward: 5009,
    quest_item: (4332, 30),
    empty_places: 5,
    empty_text: "Empty pos.",
    tele_prefix: "35560",
};

const WILD: Consts = Consts {
    hall: BEAST_FARM,
    script: HallScript::WildBeastReserve,
    prefix: "35627",
    shift: 1,
    royal_flag: 35606,
    allies: [35618, 35619, 35620, 35621, 35622],
    teleport_1: 35612,
    messenger: 35627,
    outer_doors: [21150003, 21150004],
    inner_doors: [21150001, 21150002],
    flag_coords: [
        (56963, -92211, -1303, 60611),
        (58090, -91641, -1303, 47274),
        (58908, -92556, -1303, 34450),
        (58336, -93600, -1303, 21100),
        (57152, -93360, -1303, 8400),
        (59116, -93251, -1302, 31000),
        (56432, -92864, -1303, 64000),
    ],
    center: (57762, -92696, -1359),
    quest_reward: 8293,
    quest_item: (8084, 10),
    empty_places: 6,
    empty_text: "Empty pos. ",
    tele_prefix: "35612",
};

/// `registerClan`'s fee of the `wFee` registration.
const REGISTRATION_FEE: i64 = 200_000;
/// The delay of the two lambdas and of the respawn of the flags and the warriors.
const FIVE_MINUTES_MS: i64 = 300_000;
const WARRIOR_RESPAWN_SECONDS: i32 = 10_000;

fn consts_of(script: HallScript) -> Option<&'static Consts> {
    match script {
        HallScript::BanditStronghold => Some(&BANDIT),
        HallScript::WildBeastReserve => Some(&WILD),
        _ => None,
    }
}

impl Consts {
    /// A page of the script: `prefix-NN.htm` (`shifted` pages are one higher in Wild Beast Reserve).
    fn page(&self, number: i32) -> String {
        format!("{}-{number:02}.htm", self.prefix)
    }

    fn shifted(&self, number: i32) -> i32 {
        number + self.shift
    }

    /// Java `getFlagHtml(flag)`.
    fn flag_html(&self, flag: i32) -> String {
        let red = self.royal_flag + 1;
        match flag - red {
            0..=4 => self.page(self.shifted(15 + (flag - red))),
            _ => format!("{}-{:02}a.htm", self.prefix, self.shifted(14)),
        }
    }

    /// Java `getAllyHtml(ally)`.
    fn ally_html(&self, ally: i32) -> Option<String> {
        let index = self.allies.iter().position(|a| *a == ally)?;
        Some(format!(
            "{}-{:02}a.htm",
            self.prefix,
            self.shifted(15 + i32::try_from(index).unwrap_or(0))
        ))
    }

    fn red(&self) -> i32 {
        self.royal_flag + 1
    }
}

/// `ClanData`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlagWarData {
    /// `flag`: the NPC id of the clan's flag.
    pub flag: i32,
    /// `npc`: the NPC id of the warrior the clan chose, 0 for none.
    pub npc: i32,
    /// `players`: the object ids of the clan's registered fighters.
    pub players: Vec<i32>,
    /// `playersInstance`: the ones online when the siege started.
    pub players_instance: Vec<ObjectId>,
    /// `warrior`: its spawn.
    pub warrior: Option<usize>,
    /// `flagInstance`: its spawn.
    pub flag_instance: Option<usize>,
    /// Whether the registration's row went out (kept over a re-read, D-50).
    pub written: bool,
}

/// What the script keeps.
#[derive(Debug, Default)]
pub(crate) struct FlagWar {
    /// `_data`: clan id to its data, in registration order (a `HashMap`).
    pub(crate) data: Vec<(i32, FlagWarData)>,
    /// `_winner`.
    pub(crate) winner: Option<i32>,
    /// `_firstPhase`.
    pub(crate) first_phase: bool,
}

impl FlagWar {
    pub(crate) fn data_of(&self, clan: i32) -> Option<&FlagWarData> {
        self.data.iter().find(|(c, _)| *c == clan).map(|(_, d)| d)
    }

    pub(crate) fn data_mut(&mut self, clan: i32) -> Option<&mut FlagWarData> {
        self.data.iter_mut().find(|(c, _)| *c == clan).map(|(_, d)| d)
    }

    /// `_data.keySet()` in the map's iteration order.
    fn keys(&self) -> Vec<i32> {
        let keys: Vec<i32> = self.data.iter().map(|(c, _)| *c).collect();
        java_hash_map_order(&keys).into_iter().map(|i| keys[i]).collect()
    }
}

impl World {
    fn flag_war(&self, hall: ClanHallId) -> Option<&FlagWar> {
        match self.hall_siege(hall).map(|s| &s.state) {
            Some(ScriptState::FlagWar(s)) => Some(s),
            _ => None,
        }
    }

    fn flag_war_mut(&mut self, hall: ClanHallId) -> Option<&mut FlagWar> {
        match self.hall_siege_mut(hall).map(|s| &mut s.state) {
            Some(ScriptState::FlagWar(s)) => Some(s),
            _ => None,
        }
    }

    /// The clans registered with their flag, in the map's order (for the tests and the pages).
    pub fn flag_war_clans(&self, hall: ClanHallId) -> Vec<(i32, FlagWarData)> {
        let Some(state) = self.flag_war(hall) else {
            return Vec::new();
        };
        state
            .keys()
            .into_iter()
            .filter_map(|c| state.data_of(c).map(|d| (c, d.clone())))
            .collect()
    }

    fn flag_war_write(&mut self, write: HallSiegeWrite) -> bool {
        self.siege_write(ResidenceWrite::HallSiege(write))
    }

    /// Java `registerClan(clan)`.
    fn flag_war_register_clan(&mut self, hall: ClanHallId, clan_id: i32) -> ScriptResult {
        let Some(leader) = self.clan(clan_id).map(|c| c.leader_id()) else {
            return Err(ScriptError("java.lang.NullPointerException"));
        };
        let royal = consts_of(
            self.hall_siege(hall)
                .map(|s| s.script)
                .ok_or(ScriptError("no siege"))?,
        )
        .map_or(0, |c| c.royal_flag);
        self.hall_add_attacker(hall, clan_id);
        let size = self.flag_war(hall).map_or(0, |s| s.data.len());
        let flag = royal + i32::try_from(size).unwrap_or(0);
        let mut data = FlagWarData {
            flag,
            players: vec![leader],
            ..FlagWarData::default()
        };
        let clan_written = self.flag_war_write(HallSiegeWrite::FlagWarClan {
            hall_id: hall.0,
            flag,
            clan_id,
        });
        let member_written = self.flag_war_write(HallSiegeWrite::FlagWarMember {
            hall_id: hall.0,
            clan_id,
            object_id: leader,
        });
        data.written = clan_written && member_written;
        if let Some(state) = self.flag_war_mut(hall) {
            // `_data.put(clanId, data)`: a known clan keeps its place in the map.
            match state.data_mut(clan_id) {
                Some(existing) => *existing = data,
                None => state.data.push((clan_id, data)),
            }
        }
        Ok(())
    }

    /// Java `sendRegistrationPageDate(player)`.
    fn flag_war_send_registration_page_date(&mut self, c: &Consts, hall: ClanHallId, player: ObjectId) {
        let next = self.hall_next_siege_time(hall);
        let date = java_date_string(next, self.siege_offset_at(next));
        let path = format!(
            "data/scripts/conquerablehalls/{}/{}",
            c.script.name(),
            c.page(c.shifted(14))
        );
        let html = self.hall_page_text(player, &path).replace("%nextSiege%", &date);
        // `new NpcHtmlMessage(5)`.
        if let Some(message) = NpcHtmlMessage::new(5, &html) {
            self.send_html(player, &message);
        }
    }

    // ------------------------------------------------------------------ onEvent

    /// Java `onEvent(event, npc, player)`: the text for `showResult`.
    pub(super) fn flag_war_on_event(
        &mut self,
        script: HallScript,
        player: ObjectId,
        event: &str,
    ) -> Option<String> {
        let c = consts_of(script)?;
        let hall = ClanHallId(c.hall);
        let clan = self
            .clan_of(player)
            .map(|cl| (cl.id, cl.level, cl.castle_id, cl.fort_id, cl.hideout_id));
        let leader = self.is_clan_leader(player);
        let mut htmltext = Some(event.to_owned());
        if event.starts_with("Register") {
            if !self.hall_is_registering(hall) {
                if self.hall_is_in_siege(hall) {
                    htmltext = Some(c.page(2));
                } else {
                    self.flag_war_send_registration_page_date(c, hall, player);
                    return None;
                }
            } else if clan.is_none() || !leader {
                htmltext = Some(c.page(3));
            } else if self.hall_siege(hall).is_some_and(|s| s.attackers.len() >= 5) {
                htmltext = Some(c.page(4));
            } else if clan.is_some_and(|cl| self.hall_siege_is_attacker(hall, cl.0)) {
                htmltext = Some(c.page(5));
            } else if clan.is_some_and(|cl| self.clan_hall(hall).is_some_and(|h| h.owner_id == cl.0)) {
                htmltext = Some(c.page(6));
            } else {
                let arg: Vec<&str> = event.split(' ').collect();
                if arg.len() >= 2 {
                    let clan_id = clan.map_or(0, |cl| cl.0);
                    let paid = if arg[1] == "wQuest" {
                        Some((
                            self.destroy_item_by_item_id(player, c.quest_reward, 1, true),
                            c.page(7),
                        ))
                    } else if arg[1] == "wFee" {
                        // `canPayRegistration()` is true.
                        Some((self.reduce_adena(player, REGISTRATION_FEE, true), c.page(8)))
                    } else {
                        None
                    };
                    if let Some((paid, refused)) = paid {
                        if !paid {
                            htmltext = Some(refused);
                        } else if self.flag_war_register_clan(hall, clan_id).is_ok() {
                            let flag = self
                                .flag_war(hall)
                                .and_then(|s| s.data_of(clan_id))
                                .map_or(0, |d| d.flag);
                            htmltext = Some(c.flag_html(flag));
                        } else {
                            htmltext = None;
                        }
                    }
                }
            }
        } else if event.starts_with("Select_NPC") {
            if !leader {
                htmltext = Some(c.page(9));
            } else {
                let clan_id = clan.map_or(0, |cl| cl.0);
                if self.flag_war(hall).and_then(|s| s.data_of(clan_id)).is_none() {
                    htmltext = Some(c.page(10));
                } else {
                    let var: Vec<&str> = event.split(' ').collect();
                    if var.len() >= 2 {
                        let id = var[1].parse::<i32>().unwrap_or_else(|_| {
                            warn!(
                                "{}->select_clan_npc->Wrong mahum warrior id: {}",
                                c.script.name(),
                                var[1]
                            );
                            0
                        });
                        if id > 0 {
                            htmltext = c.ally_html(id);
                            if htmltext.is_some() {
                                if let Some(d) = self.flag_war_mut(hall).and_then(|s| s.data_mut(clan_id)) {
                                    d.npc = id;
                                }
                                self.flag_war_write(HallSiegeWrite::FlagWarNpc { npc: id, clan_id });
                            }
                        }
                    } else {
                        warn!(
                            "{} Siege: Not enough parameters to save clan npc",
                            c.script.name()
                        );
                    }
                }
            }
        } else if event.starts_with("View") {
            htmltext = match clan {
                None => Some(c.page(10)),
                Some(cl) => match self.flag_war(hall).and_then(|s| s.data_of(cl.0)) {
                    None => Some(c.page(3)),
                    Some(d) if d.npc == 0 => Some(c.page(11)),
                    Some(d) => c.ally_html(d.npc),
                },
            };
        } else if event.starts_with("Attackers") {
            if self.hall_is_registering(hall) {
                self.flag_war_send_registration_page_date(c, hall, player);
                return None;
            }
            htmltext = self.flag_war_attackers_page(c, hall, player);
        } else if event.starts_with("CheckQuest") {
            htmltext = Some(self.flag_war_check_quest(c, hall, player, clan, leader));
        }
        htmltext
    }

    /// The `Attackers` event: the list of the clans.
    // NOTE(java-quirk): every line shows the name of the *player's* clan, not the attacker's.
    fn flag_war_attackers_page(&mut self, c: &Consts, hall: ClanHallId, player: ObjectId) -> Option<String> {
        let path = format!(
            "data/scripts/conquerablehalls/{}/{}",
            c.script.name(),
            c.page(c.shifted(13))
        );
        let mut html = self.get_htm(player, &path)?;
        // `clan.getName()` of a player without a clan throws.
        let own_name = self.clan_of(player).map(|cl| cl.name.clone())?;
        let state = self.flag_war(hall)?;
        let rows: Vec<(i32, usize)> = state
            .keys()
            .into_iter()
            .filter_map(|clan| state.data_of(clan).map(|d| (clan, d.players.len())))
            .collect();
        let total = rows.len();
        let mut i = 1;
        for (clan, members) in rows {
            if self.clan(clan).is_none() {
                continue;
            }
            html = html.replace(&format!("%clan{i}%"), &own_name);
            html = html.replace(&format!("%clanMem{i}%"), &members.to_string());
            i += 1;
        }
        if total < 5 {
            for n in i..c.empty_places {
                html = html.replace(&format!("%clan{n}%"), c.empty_text);
                html = html.replace(&format!("%clanMem{n}%"), "0");
            }
        }
        Some(html)
    }

    /// The `CheckQuest` event.
    fn flag_war_check_quest(
        &mut self,
        c: &Consts,
        hall: ClanHallId,
        player: ObjectId,
        clan: Option<(i32, i32, i32, i32, i32)>,
        leader: bool,
    ) -> String {
        match clan {
            None => c.page(c.shifted(22)),
            Some((_, level, ..)) if level < 4 => c.page(c.shifted(22)),
            Some(_) if !leader => c.page(c.shifted(23)),
            Some((_, _, castle, fort, hideout)) if hideout > 0 || fort > 0 || castle > 0 => {
                c.page(c.shifted(24))
            }
            Some(_) if !self.hall_is_waiting_battle(hall) => {
                self.flag_war_send_registration_page_date(c, hall, player);
                String::new()
            }
            Some(_) if self.inventory_item_by_item_id(player, c.quest_reward).is_some() => {
                c.page(c.shifted(25))
            }
            Some(_) => {
                let (item, needed) = c.quest_item;
                if self.inventory_item_count(player, item) >= needed {
                    format!("{}-{:02}a.htm", c.prefix, c.shifted(21))
                } else {
                    c.page(c.shifted(21))
                }
            }
        }
    }

    // ------------------------------------------------------------------ onFirstTalk

    /// Java `onFirstTalk(npc, player)`: the text for `showResult` (`None` for Java's `""`).
    pub(super) fn flag_war_first_talk(
        &mut self,
        script: HallScript,
        npc: ObjectId,
        npc_id: i32,
        player: ObjectId,
    ) -> Option<String> {
        let c = consts_of(script)?;
        let hall = ClanHallId(c.hall);
        if npc_id == c.messenger {
            let clan = self.clan_id_of(player);
            if !self.hall_siege_is_attacker(hall, clan) {
                let owner = self.clan_hall(hall).map_or(0, |h| h.owner_id);
                let name = self
                    .clan(owner)
                    .map_or_else(|| "no owner".to_owned(), |cl| cl.name.clone());
                let path = format!("data/scripts/conquerablehalls/{}/{}", c.script.name(), c.page(0));
                let html = self.hall_page_text(player, &path).replace("%clanName%", &name);
                if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
                    self.send_html(player, &message);
                }
                None
            } else {
                Some(c.page(1))
            }
        } else {
            let index = npc_id - c.teleport_1;
            let first_phase = self.flag_war(hall).is_some_and(|s| s.first_phase);
            if index == 0 && first_phase {
                return Some(format!("{}-00.htm", c.tele_prefix));
            }
            if !self.hall_check_teleport_task(hall, index) {
                warn!("{}: no teleport zone {index}.", c.script.name());
                return None;
            }
            Some(format!("{}-01.htm", c.tele_prefix))
        }
    }

    // ------------------------------------------------------------------ spawns

    /// Java `onSpawn(npc)`: the warriors walk to the middle.
    pub(super) fn flag_war_on_spawn(&mut self, script: HallScript, npc: ObjectId) {
        if let Some(c) = consts_of(script) {
            self.set_intention_move_to(npc, c.center.0, c.center.1, c.center.2);
        }
    }

    /// Java `addSpawn(templateId, loc)` followed by `setRespawnDelay(10000)` and `startRespawn()`.
    fn flag_war_add_spawn(&mut self, npc_id: i32, loc: (i32, i32, i32, i32)) -> (usize, Option<ObjectId>) {
        let spawn = self.fort_new_spawn(npc_id, (loc.0, loc.1, loc.2), loc.3, WARRIOR_RESPAWN_SECONDS);
        let npc = self.fort_do_spawn(spawn);
        self.start_respawn(spawn);
        (spawn, npc)
    }

    /// Java `doSpawns(clanId, data)`.
    fn flag_war_do_spawns(&mut self, c: &Consts, hall: ClanHallId, clan_id: i32) {
        let Some(data) = self.flag_war(hall).and_then(|s| s.data_of(clan_id)).cloned() else {
            return;
        };
        let known = |world: &World, id: i32| world.npc_world.template(id).is_some();
        if !known(self, data.flag) {
            warn!(
                "{}: Flag NpcTemplate[{}] does not exist!",
                c.script.name(),
                data.flag
            );
            warn!("{}: Could not make clan spawns: null", c.script.name());
            return;
        }
        if !known(self, data.npc) {
            warn!(
                "{}: Ally NpcTemplate[{}] does not exist!",
                c.script.name(),
                data.npc
            );
            warn!("{}: Could not make clan spawns: null", c.script.name());
            return;
        }
        let first_phase = self.flag_war(hall).is_some_and(|s| s.first_phase);
        let owner = self.clan_hall(hall).map_or(0, |h| h.owner_id);
        // NOTE(java-quirk): the royal flag (the first clan to register) has no coordinate in the
        // first phase: Java's `FLAG_COORDS[-1]` throws and nothing spawns for the clan.
        let index = if first_phase {
            data.flag - c.red()
        } else if clan_id == owner {
            5
        } else {
            6
        };
        let Some(loc) = usize::try_from(index)
            .ok()
            .and_then(|i| c.flag_coords.get(i))
            .copied()
        else {
            warn!(
                "{}: Could not make clan spawns: Index {index} out of bounds for length 7",
                c.script.name()
            );
            return;
        };
        let (flag_spawn, _) = self.flag_war_add_spawn(data.flag, loc);
        let (warrior_spawn, warrior) = self.flag_war_add_spawn(data.npc, loc);
        if let Some(warrior) = warrior {
            self.clanhall.siege.allies.insert(warrior, data.players.clone());
        }
        if let Some(d) = self.flag_war_mut(hall).and_then(|s| s.data_mut(clan_id)) {
            d.flag_instance = Some(flag_spawn);
            d.warrior = Some(warrior_spawn);
        }
    }

    /// Java `doUnSpawns(data)`.
    fn flag_war_do_unspawns(&mut self, data: &FlagWarData) {
        for spawn in [data.flag_instance, data.warrior].into_iter().flatten() {
            self.stop_respawn(spawn);
            if let Some(npc) = self.last_spawn(spawn) {
                self.delete_npc(npc);
            }
        }
    }

    /// Java `removeParticipant(clanId, teleport)`.
    fn flag_war_remove_participant(&mut self, hall: ClanHallId, clan_id: i32, teleport: bool) {
        let Some(state) = self.flag_war_mut(hall) else {
            return;
        };
        let Some(position) = state.data.iter().position(|(c, _)| *c == clan_id) else {
            return;
        };
        let (_, data) = state.data.remove(position);
        self.flag_war_do_unspawns(&data);
        if teleport {
            for player in data.players_instance {
                if self.players.contains_key(&player) {
                    self.tele_to_location_where(player, TeleportWhereType::Town);
                }
            }
        }
    }

    /// Java `clearTables()`.
    fn flag_war_clear_tables(&mut self, hall: ClanHallId) {
        self.flag_war_write(HallSiegeWrite::FlagWarClear { hall_id: hall.0 });
    }

    // ------------------------------------------------------------------ onKill

    /// Java `onKill(npc, killer, isSummon)`.
    pub(super) fn flag_war_on_kill(&mut self, script: HallScript, npc_id: i32) {
        let Some(c) = consts_of(script) else {
            return;
        };
        let hall = ClanHallId(c.hall);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        if let Err(e) = self.flag_war_on_kill_inner(c, hall, npc_id) {
            warn!("{}: onKill failed: {}", c.script.name(), e.0);
        }
    }

    fn flag_war_on_kill_inner(&mut self, c: &Consts, hall: ClanHallId, npc_id: i32) -> ScriptResult {
        // `for (int keys : _data.keySet()) if (_data.get(keys).npc == npcId) removeParticipant(...)`
        let keys = self.flag_war(hall).map(FlagWar::keys).unwrap_or_default();
        for (position, key) in keys.iter().enumerate() {
            if self
                .flag_war(hall)
                .and_then(|s| s.data_of(*key))
                .is_some_and(|d| d.npc == npc_id)
            {
                self.flag_war_remove_participant(hall, *key, true);
                // NOTE(java-quirk): the map changed under the iterator: the next `next()` throws a
                // `ConcurrentModificationException` unless this was the last key.
                if position + 1 < keys.len() {
                    return Err(ScriptError("java.util.ConcurrentModificationException"));
                }
            }
        }
        let clan_ids: Vec<i32> = self.flag_war(hall).map(FlagWar::keys).unwrap_or_default();
        let (first_phase, owner) = (
            self.flag_war(hall).is_some_and(|s| s.first_phase),
            self.clan_hall(hall).map_or(0, |h| h.owner_id),
        );
        if first_phase {
            let first = clan_ids
                .first()
                .copied()
                .ok_or(ScriptError("java.lang.IndexOutOfBoundsException"))?;
            let first_npc = self
                .flag_war(hall)
                .and_then(|s| s.data_of(first))
                .map_or(0, |d| d.npc);
            if (clan_ids.len() == 1 && owner <= 0) || first_npc == 0 {
                self.flag_war_finish(hall);
            } else if clan_ids.len() == 2 && owner > 0 {
                self.hall_cancel_siege_task(hall);
                if let Some(state) = self.flag_war_mut(hall) {
                    state.first_phase = false;
                }
                self.hall_set_siege_zone_active_only(hall, false);
                for door in c.inner_doors {
                    self.hall_open_close_door(hall, door, true);
                }
                let all: Vec<FlagWarData> = self
                    .flag_war(hall)
                    .map(|s| s.data.iter().map(|(_, d)| d.clone()).collect())
                    .unwrap_or_default();
                for data in all {
                    self.flag_war_do_unspawns(&data);
                }
                self.hall_schedule(HallSiegeTimer::FlagWarSecondPhase(hall), FIVE_MINUTES_MS);
            }
        } else {
            let first = clan_ids
                .first()
                .copied()
                .ok_or(ScriptError("java.lang.IndexOutOfBoundsException"))?;
            if let Some(siege) = self.hall_siege_mut(hall) {
                siege.mission_accomplished = true;
            }
            if let Some(state) = self.flag_war_mut(hall) {
                state.winner = Some(first);
            }
            self.flag_war_remove_participant(hall, first, false);
            self.hall_end_siege(hall);
        }
        Ok(())
    }

    /// `_missionAccomplished = true; cancelSiegeTask(); endSiege();`
    fn flag_war_finish(&mut self, hall: ClanHallId) {
        if let Some(siege) = self.hall_siege_mut(hall) {
            siege.mission_accomplished = true;
        }
        self.hall_cancel_siege_task(hall);
        self.hall_end_siege(hall);
    }

    /// `ThreadPool.schedule(() -> { inner doors close; doSpawns; zone active }, 300000)`.
    pub(crate) fn flag_war_second_phase_due(&mut self, hall: ClanHallId) {
        let Some(c) = self.hall_siege(hall).and_then(|s| consts_of(s.script)) else {
            return;
        };
        for door in c.inner_doors {
            self.hall_open_close_door(hall, door, false);
        }
        let keys = self.flag_war(hall).map(FlagWar::keys).unwrap_or_default();
        for clan in keys {
            self.flag_war_do_spawns(c, hall, clan);
        }
        self.hall_set_siege_zone_active_only(hall, true);
    }

    /// `ThreadPool.schedule(() -> { outer doors close; banishNonSiegeParticipants; startSiege() },
    /// 300000)` of `startSiege()`.
    pub(crate) fn flag_war_outer_doors_due(&mut self, hall: ClanHallId) {
        let Some(c) = self.hall_siege(hall).and_then(|s| consts_of(s.script)) else {
            return;
        };
        for door in c.outer_doors {
            self.hall_open_close_door(hall, door, false);
        }
        self.hall_banish_non_siege_participants(hall);
        self.hall_start_siege(hall);
    }

    /// `getZone().setActive(value)` of the siege zone without the rest of `updateSiegeZone`.
    fn hall_set_siege_zone_active_only(&mut self, hall: ClanHallId, active: bool) {
        self.hall_update_siege_zone(hall, active);
    }

    // ------------------------------------------------------------------ the inner spawns

    /// Java `getInnerSpawnLoc(player)`: where a participant restarts inside the hall.
    // NOTE(java-quirk): `Player.getId()` is the class id, so the owner test and the lookup of the
    // clan data never match a real clan.
    pub(crate) fn flag_war_inner_spawn_loc(
        &self,
        hall: ClanHallId,
        player: ObjectId,
    ) -> Result<Option<Location>, ScriptError> {
        let Some(c) = self.hall_siege(hall).and_then(|s| consts_of(s.script)) else {
            return Ok(None);
        };
        let id = self.players.get(&player).map_or(0, |p| p.data.class_id);
        let owner = self.clan_hall(hall).map_or(0, |h| h.owner_id);
        if id == owner {
            return Ok(self.hall_zone_spawns(hall).first().copied());
        }
        let Some(data) = self.flag_war(hall).and_then(|s| s.data_of(id)) else {
            return Ok(None);
        };
        let index = data.flag - c.red();
        if !(0..=4).contains(&index) {
            return Err(ScriptError("java.lang.ArrayIndexOutOfBoundsException"));
        }
        Ok(self
            .hall_zone_challenger_spawns(hall)
            .get(usize::try_from(index).unwrap_or(0))
            .copied())
    }
}

// ---------------------------------------------------------------------------- the engine hooks

/// Java `prepareOwner()`.
pub(super) fn prepare_owner(world: &mut World, hall: ClanHallId) -> ScriptResult {
    let owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
    if owner > 0 {
        world.flag_war_register_clan(hall, owner)?;
    }
    world.residence_banish_foreigners(ResidenceRef::ClanHall(hall));
    world.hall_broadcast_registration_ended(hall);
    world.hall_set_status(hall, SiegeStatus::WaitingBattle);
    world.hall_set_task(hall, HallSiegeTimer::SiegeStarts(hall), 3_600_000);
    Ok(())
}

/// Java `startSiege()`.
pub(super) fn start_siege(world: &mut World, hall: ClanHallId) -> ScriptResult {
    let Some(c) = world.hall_siege(hall).and_then(|s| consts_of(s.script)) else {
        return Ok(());
    };
    if world.hall_siege(hall).is_none_or(|s| s.attackers.len() < 2) {
        world.hall_on_siege_ends(hall)?;
        world.hall_clear_attackers(hall);
        world.hall_update_next_siege(hall);
        world.hall_broadcast_lack_of_interest(hall);
        return Ok(());
    }
    for door in c.outer_doors {
        world.hall_open_close_door(hall, door, true);
    }
    let owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
    if owner > 0 {
        let loc = world
            .hall_zone_spawns(hall)
            .first()
            .copied()
            .ok_or(ScriptError("java.lang.IndexOutOfBoundsException"))?;
        for member in world.clan_online_members(owner, 0) {
            world.tele_to_location(member, crate::teleport::TeleportTarget::at(loc.x, loc.y, loc.z));
        }
    }
    // NOTE(java-quirk): the lambda calls `startSiege()` of this script again, see the module.
    world.hall_schedule(HallSiegeTimer::FlagWarOuterDoors(hall), FIVE_MINUTES_MS);
    Ok(())
}

/// Java `onSiegeStarts()`.
pub(super) fn on_siege_starts(world: &mut World, hall: ClanHallId) {
    let Some(c) = world.hall_siege(hall).and_then(|s| consts_of(s.script)) else {
        return;
    };
    let keys = world.flag_war(hall).map(FlagWar::keys).unwrap_or_default();
    for clan in keys {
        world.flag_war_do_spawns(c, hall, clan);
        // `fillPlayerList(data)`.
        let online: Vec<ObjectId> = world
            .flag_war(hall)
            .and_then(|s| s.data_of(clan))
            .map(|d| {
                d.players
                    .iter()
                    .map(|p| ObjectId(*p))
                    .filter(|p| world.players.contains_key(p))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(d) = world.flag_war_mut(hall).and_then(|s| s.data_mut(clan)) {
            d.players_instance.extend(online);
        }
    }
}

/// Java `onSiegeEnds()`: the participants go (the owner's stay), the tables are cleared.
pub(super) fn on_siege_ends(world: &mut World, hall: ClanHallId) -> ScriptResult {
    let keys = world.flag_war(hall).map(FlagWar::keys).unwrap_or_default();
    let owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
    for (position, clan) in keys.iter().enumerate() {
        world.flag_war_remove_participant(hall, *clan, owner != *clan);
        // NOTE(java-quirk): the map changed under the iterator, see `onKill`.
        if position + 1 < keys.len() {
            return Err(ScriptError("java.util.ConcurrentModificationException"));
        }
    }
    world.flag_war_clear_tables(hall);
    Ok(())
}

/// The state of a script at startup (`loadAttackers` and the constructor).
pub(super) fn new_state(
    world: &mut World,
    script: HallScript,
    rows: &[FlagWarRecord],
) -> (FlagWar, Vec<i32>) {
    let hall = ClanHallId(script.hall_id());
    let mut state = FlagWar::default();
    let mut attackers = Vec::new();
    for row in rows {
        if world.clan(row.clan_id).is_none() {
            warn!(
                "{}: Loaded an unexistent clan as attacker! Clan Id: {}",
                script.name(),
                row.clan_id
            );
            continue;
        }
        attackers.push(row.clan_id);
        state.data.push((
            row.clan_id,
            FlagWarData {
                flag: row.flag,
                npc: row.npc,
                players: row.members.clone(),
                written: true,
                ..FlagWarData::default()
            },
        ));
    }
    let owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
    state.winner = Some(owner).filter(|o| world.clan(*o).is_some());
    (state, attackers)
}

/// D-50: the flag war tables as the database has them now. A registration this world holds
/// without a row (the switch was off) stays.
pub(super) fn apply_rows(world: &mut World, hall: ClanHallId, rows: &[FlagWarRecord]) {
    let Some(state) = world.flag_war_mut(hall) else {
        return;
    };
    state.data.retain(|(_, d)| !d.written);
    let mut added = Vec::new();
    for row in rows {
        if state.data_of(row.clan_id).is_some() {
            continue;
        }
        state.data.push((
            row.clan_id,
            FlagWarData {
                flag: row.flag,
                npc: row.npc,
                players: row.members.clone(),
                written: true,
                ..FlagWarData::default()
            },
        ));
        added.push(row.clan_id);
    }
    let keep: Vec<i32> = state.data.iter().map(|(c, _)| *c).collect();
    if let Some(siege) = world.hall_siege_mut(hall) {
        siege.attackers.retain(|a| keep.contains(&a.clan_id));
    }
    for clan in added {
        if world.clan(clan).is_some() && !world.hall_siege_is_attacker(hall, clan) {
            world.hall_add_attacker(hall, clan);
        }
    }
}
