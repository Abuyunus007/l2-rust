//! The fortress NPC classes: `FortDoorman`, `FortLogistics`, `FortCommander` (the foreman,
//! `FortManager`, is `manager.rs`).
// Java: entity/actor/instance/{FortDoorman,Doorman,FortLogistics,FortCommander}.java
// Java: entity/actor/Npc.java (isMyLord)

use l2_protocol_game::server::{ActionFailed, ChatType, NpcSay};
use l2_util::ObjectId;
use tracing::warn;

use super::manager::{Words, parse_int};
use super::{FortThen, FortTimer, LogisticsCommand};
use crate::clan::model::ClanAccess;
use crate::npc::NpcClass;
use crate::residence::{FortId, ResidenceRef, ResidenceWrite, Writes};
use crate::timer::TimerKind;
use crate::world::World;
use l2_protocol_game::NpcStringId;

/// `FortLogistics.SUPPLY_BOX_IDS`: the box of each supply level.
const SUPPLY_BOX_IDS: [i32; 21] = [
    35665, 35697, 35734, 35766, 35803, 35834, 35866, 35903, 35935, 35973, 36010, 36042, 36080, 36117, 36148,
    36180, 36218, 36256, 36293, 36325, 36363,
];

/// The item a clan gets for its blood oaths (`FortLogistics`: 9910).
const BLOOD_OATH_ITEM: i32 = 9910;

/// How long a commander is silent after it spoke (`ScheduleTalkTask`, ms).
const COMMANDER_TALK_PAUSE_MS: u64 = 10_000;

impl World {
    fn fort_npc_type(&self, npc: ObjectId) -> Option<&str> {
        let n = self.npcs.get(&npc)?;
        Some(self.npc_world.template(n.npc_id)?.npc_type.as_str())
    }

    fn fort_npc_id(&self, npc: ObjectId) -> i32 {
        self.npcs.get(&npc).map_or(0, |n| n.npc_id)
    }

    /// Java `Npc.isMyLord(player)`.
    // NOTE(java-quirk): the castle and fortress ids of the NPC are -1 when it has none, and the
    // comparison is `||`: a clan leader whose clan owns the castle nearest to the NPC passes at a
    // fortress that is not his.
    pub(crate) fn fort_is_my_lord(&self, npc: ObjectId, player: ObjectId) -> bool {
        if !self.is_clan_leader(player) {
            return false;
        }
        let castle = self.npc_castle(npc).map_or(-1, |c| c.0);
        let fort = self.npc_fort(npc).map_or(-1, |f| f.0);
        self.clan_of(player)
            .is_some_and(|c| c.castle_id == castle || c.fort_id == fort)
    }

    // ------------------------------------------------------------------ the first click

    /// The windows of the NPCs this card owns (`showChatWindow(player)` of `FortManager`,
    /// `FortDoorman` and `FortLogistics`, `onFirstTalk` of `FortressSiegeManager` and
    /// `FortressArcherCaptain`). `false` for any other NPC.
    pub(crate) fn fort_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        match self.fort_npc_type(npc) {
            Some("FortManager") => {
                self.fort_manager_show_chat_window(npc, player);
                return true;
            }
            Some("FortDoorman") => {
                self.fort_doorman_show_chat_window(npc, player);
                return true;
            }
            Some("FortLogistics") => {
                self.fort_logistics_show_message_window(npc, player, 0);
                return true;
            }
            _ => {}
        }
        self.fort_script_first_talk(npc, player)
    }

    /// The `onBypassFeedback` of the fortress NPC classes. `false` for any other NPC.
    pub(crate) fn fort_npc_bypass_feedback(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) -> bool {
        match self.fort_npc_type(npc) {
            Some("FortManager") => self.fort_manager_bypass_feedback(npc, player, command),
            Some("FortDoorman") => self.fort_doorman_bypass_feedback(npc, player, command),
            Some("FortLogistics") => self.fort_logistics_bypass_feedback(npc, player, command),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ FortDoorman

    /// Java `FortDoorman.isOwnerClan(player)`.
    fn fort_doorman_is_owner_clan(&self, npc: ObjectId, player: ObjectId) -> bool {
        let clan = self.clan_id_of(player);
        self.clan_of(player).is_some()
            && self
                .npc_fort(npc)
                .and_then(|f| self.fort(f))
                .is_some_and(|f| f.owner_id != 0 && f.owner_id == clan)
    }

    /// Java `FortDoorman.isUnderSiege()`.
    fn fort_doorman_is_under_siege(&self, npc: ObjectId) -> bool {
        self.npc_fort(npc).is_some_and(|f| self.fort_zone_is_active(f))
    }

    /// Java `FortDoorman.showChatWindow(player)`.
    fn fort_doorman_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) {
        self.send(player, &ActionFailed);
        let id = self.fort_npc_id(npc);
        let page = if !self.fort_doorman_is_owner_clan(npc, player) {
            format!("data/html/doorman/{id}-no.htm")
        } else if self.fort_doorman_is_under_siege(npc) {
            format!("data/html/doorman/{id}-busy.htm")
        } else {
            format!("data/html/doorman/{id}.htm")
        };
        self.fort_page(npc, player, &page, &[]);
    }

    /// Java `Doorman.onBypassFeedback(player, command)` with the fortress's overrides.
    fn fort_doorman_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        if command.starts_with("Chat") {
            self.fort_doorman_show_chat_window(npc, player);
            return;
        }
        if command.starts_with("open_doors") || command.starts_with("close_doors") {
            let open = command.starts_with("open_doors");
            if self.fort_doorman_is_owner_clan(npc, player) {
                if self.fort_doorman_is_under_siege(npc) {
                    // `cannotManageDoors`.
                    self.send(player, &ActionFailed);
                    let id = self.fort_npc_id(npc);
                    self.fort_page(npc, player, &format!("data/html/doorman/{id}-busy.htm"), &[]);
                } else if self.fort_doorman_doors(npc, player, command, open).is_none() {
                    warn!("Exception processing bypass from {player}: {command} (fortress doorman)");
                }
            }
            return;
        }
        if command.starts_with("tele") {
            if self.fort_doorman_is_owner_clan(npc, player) {
                let npc_id = self.fort_npc_id(npc);
                let teleporters = self.talk.teleporters.clone();
                let holder = teleporters
                    .as_ref()
                    .and_then(|d| d.get_holder(npc_id, l2_data::TeleportType::Other.name()));
                if let Some(holder) = holder {
                    // `Integer.parseInt(command.substring(5).trim())`.
                    let Some(location) = command.get(5..).and_then(|rest| parse_int(rest.trim())) else {
                        warn!("Exception processing bypass from {player}: {command} (fortress doorman)");
                        return;
                    };
                    self.do_teleport(npc, player, holder, location);
                }
            }
            return;
        }
        self.generic_npc_bypass_feedback(npc, player, command);
    }

    /// `FortDoorman.openDoors` / `closeDoors`: "open_doors 5, 24180001, 24180002". The first
    /// number is skipped (`st.nextToken()`).
    fn fort_doorman_doors(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
        open: bool,
    ) -> Option<()> {
        let skip = if open { 10 } else { 11 };
        let rest = command.get(skip..)?;
        let mut tokens = rest.split([',', ' ']).filter(|t| !t.is_empty());
        tokens.next()?;
        let fort = self.npc_fort(npc)?;
        for token in tokens {
            let door = parse_int(token)?;
            self.residence_open_close_door(player, ResidenceRef::Fort(fort), door, open);
        }
        Some(())
    }

    // ------------------------------------------------------------------ FortLogistics

    /// Java `FortLogistics.showMessageWindow(player, value)`.
    pub(crate) fn fort_logistics_show_message_window(&mut self, npc: ObjectId, player: ObjectId, value: i32) {
        self.send(player, &ActionFailed);
        let filename = if value == 0 {
            "data/html/fortress/logistics.htm".to_owned()
        } else {
            format!("data/html/fortress/logistics-{value}.htm")
        };
        let owner = self
            .npc_fort(npc)
            .and_then(|f| self.fort(f))
            .map(|f| f.owner_id)
            .filter(|c| *c != 0);
        let clan_name = owner
            .and_then(|c| self.clan(c))
            .map_or_else(|| "NPC".to_owned(), |c| c.name.clone());
        self.fort_page(npc, player, &filename, &[("%clanname%", clan_name)]);
    }

    /// Java `FortLogistics.onBypassFeedback(player, command)`: the commands that hand out
    /// something (the blood oath reward, the supply box) read the fortress and the clan again
    /// first (D-50).
    fn fort_logistics_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        if self.players.get(&player).and_then(|p| p.talk.last_folk_npc) != Some(npc) {
            return;
        }
        let mut st = Words::new(command);
        let Some(actual) = st.next() else {
            return;
        };
        let kind = match actual.to_ascii_lowercase().as_str() {
            "rewards" => LogisticsCommand::Rewards,
            "blood" => LogisticsCommand::Blood,
            "supplylvl" => LogisticsCommand::SupplyLevel,
            "supply" => LogisticsCommand::Supply,
            _ => {
                // `super.onBypassFeedback(player, command)`.
                self.generic_npc_bypass_feedback(npc, player, command);
                return;
            }
        };
        match self.npc_fort(npc) {
            Some(fort) => self.fort_request_siege_reread(fort, FortThen::Logistics { player, npc, kind }),
            None => warn!("{player}: the logistics officer {npc} stands at no fortress."),
        }
    }

    /// The rest of the `FortLogistics` commands once the rows are fresh.
    pub(super) fn fort_logistics_command(&mut self, npc: ObjectId, player: ObjectId, kind: LogisticsCommand) {
        if !self.players.contains_key(&player) || !self.npcs.contains_key(&npc) {
            return;
        }
        let Some(fort) = self.npc_fort(npc) else {
            return;
        };
        let page = |name: &str| format!("data/html/fortress/{name}.htm");
        match kind {
            LogisticsCommand::Rewards => {
                if self.fort_is_my_lord(npc, player) {
                    let blood = self.clan_of(player).map_or(0, |c| c.blood_oath_count);
                    self.fort_page(
                        npc,
                        player,
                        &page("logistics-rewards"),
                        &[("%bloodoath%", blood.to_string())],
                    );
                } else {
                    self.fort_page(npc, player, &page("logistics-noprivs"), &[]);
                }
            }
            LogisticsCommand::Blood => {
                if self.fort_is_my_lord(npc, player) {
                    let clan = self.clan_id_of(player);
                    let blood = self.clan_of(player).map_or(0, |c| c.blood_oath_count);
                    if blood > 0 {
                        self.add_item(player, BLOOD_OATH_ITEM, i64::from(blood), true);
                        // `clan.resetBloodOathCount()`: a player's own action, written.
                        if let Some(c) = self.clan_mut(clan) {
                            c.blood_oath_count = 0;
                        }
                        self.write_residence(
                            Writes::Player,
                            ResidenceWrite::FortSiege(super::FortSiegeWrite::BloodOathCount {
                                clan_id: clan,
                                count: 0,
                            }),
                        );
                        self.fort_page(npc, player, &page("logistics-blood"), &[]);
                    } else {
                        self.fort_page(npc, player, &page("logistics-noblood"), &[]);
                    }
                } else {
                    self.fort_page(npc, player, &page("logistics-noprivs"), &[]);
                }
            }
            LogisticsCommand::SupplyLevel => {
                let (state, supply) = self.fort(fort).map_or((0, 0), |f| (f.state, f.supply_level));
                if state == 2 {
                    if self.is_clan_leader(player) {
                        self.fort_page(
                            npc,
                            player,
                            &page("logistics-supplylvl"),
                            &[("%supplylvl%", supply.to_string())],
                        );
                    } else {
                        self.fort_page(npc, player, &page("logistics-noprivs"), &[]);
                    }
                } else {
                    // TODO(java): Missing HTML?
                    self.fort_page(npc, player, &page("logistics-1"), &[]);
                }
            }
            LogisticsCommand::Supply => self.fort_logistics_supply(npc, player, fort),
        }
    }

    /// The `supply` command: the box of the fortress's supply level appears next to the officer.
    fn fort_logistics_supply(&mut self, npc: ObjectId, player: ObjectId, fort: FortId) {
        let page = |name: &str| format!("data/html/fortress/{name}.htm");
        if !self.fort_is_my_lord(npc, player) {
            self.fort_page(npc, player, &page("logistics-noprivs"), &[]);
            return;
        }
        if self.fort_siege_in_progress(fort) {
            self.fort_page(npc, player, &page("logistics-siege"), &[]);
            return;
        }
        let level = self.fort(fort).map_or(0, |f| f.supply_level);
        if level > 0 {
            // Spawn the box.
            let box_id = usize::try_from(level - 1)
                .ok()
                .and_then(|i| SUPPLY_BOX_IDS.get(i))
                .copied();
            let Some(box_id) = box_id else {
                // `SUPPLY_BOX_IDS[level - 1]` out of range.
                warn!("{fort}: no supply box for level {level}.");
                return;
            };
            let Some((x, y, z)) = self.position_of(npc) else {
                return;
            };
            self.spawn_npc_at(box_id, x - 23, y + 41, z, 0);
            self.fort_set_supply_level(fort, 0);
            // `saveFortVariables()`: the row of the supply level, the player's own action.
            self.fort_save_variables(fort, Writes::Player);
            self.fort_page(npc, player, &page("logistics-supply"), &[]);
        } else {
            self.fort_page(npc, player, &page("logistics-nosupply"), &[]);
        }
    }

    // ------------------------------------------------------------------ FortCommander

    /// Java `FortCommander.isAutoAttackable(attacker)`: a player, during the siege of the
    /// fortress, who is not its owner's.
    pub(crate) fn fort_commander_auto_attackable(&self, commander: ObjectId, attacker: ObjectId) -> bool {
        if !self.is_player(attacker) {
            return false;
        }
        let Some(fort) = self.npc_fort(commander) else {
            return false;
        };
        fort.0 > 0
            && self.fort_siege_in_progress(fort)
            && !self.fort_check_is_defender(fort, self.clan_id_of(attacker))
    }

    /// Java `Defender.isAutoAttackable(attacker)` for a defender that stands in a fortress (the
    /// `_fort` of `Defender.onSpawn`): during the siege, by anybody who is not a defender
    /// registered on this field, nor an attacker of the territory the fortress belongs to.
    /// A defender of a castle or a siegable hall is answered by `siege_npc_auto_attackable`.
    pub(crate) fn fort_defender_auto_attackable(&self, defender: ObjectId, attacker: ObjectId) -> bool {
        let Some((x, y, z)) = self.position_of(defender) else {
            return false;
        };
        let Some(fort) = self.fort_at(x, y, z) else {
            return false;
        };
        if !self.fort_zone_is_active(fort) {
            return false;
        }
        // `attacker.isPlayable()`, then `asPlayer()`.
        let Some(player) = self.acting_player(attacker) else {
            return false;
        };
        let state = self.fort_siege_state_of(player);
        // An attacker of the Territory War is no enemy of his territory's fortress (M7-TW-01).
        (state == 2 && !self.is_registered_on_this_siege_field(player, fort.0))
            || (state == 1 && !self.is_ally_field(player, fort.0))
            || state == 0
    }

    /// The speech of `FortCommander.addDamage(creature, damage, skill)`. Returns the attacker the
    /// damage goes on with (the owner of a summon for the second commander).
    pub(crate) fn fort_commander_add_damage(&mut self, npc: ObjectId, attacker: ObjectId) -> ObjectId {
        if self.npcs.get(&npc).map(|n| n.class) != Some(NpcClass::FortCommander) {
            return attacker;
        }
        let mut attacker = attacker;
        let Some(spawn) = self.npcs.get(&npc).and_then(|n| n.spawn) else {
            return attacker;
        };
        if self.fort.silenced_commanders.contains(&npc) {
            return attacker;
        }
        let Some(fort) = self.npc_fort(npc) else {
            return attacker;
        };
        let spawn_npc_id = self.spawn_def(spawn).map_or(0, |s| s.npc_id);
        let npc_id = self.fort_npc_id(npc);
        let list = self.fort.commander_lists.get(&fort).cloned().unwrap_or_default();
        for holder in list.iter().filter(|h| h.npc_id == spawn_npc_id) {
            let string = match holder.message_id {
                1 => Some(NpcStringId::AttackingTheEnemySReinforcementsIsNecessaryTimeToDie),
                2 => {
                    if self.is_summon(attacker)
                        && let Some(owner) = self.summon_owner(attacker)
                    {
                        attacker = owner;
                    }
                    Some(NpcStringId::EveryoneConcentrateYourAttacksOnS1ShowTheEnemyYourResolve)
                }
                3 => Some(NpcStringId::SpiritOfFireUnleashYourPowerBurnTheEnemy),
                _ => None,
            };
            let Some(string) = string else {
                continue;
            };
            let parameters = if string.param_count() == 1 {
                vec![self.fort_creature_name(attacker)]
            } else {
                Vec::new()
            };
            self.broadcast_packet(
                npc,
                &NpcSay {
                    object_id: npc.0,
                    text_type: ChatType::NpcShout as i32,
                    npc_id,
                    npc_string: string as i32,
                    text: String::new(),
                    parameters,
                },
            );
            self.fort.silenced_commanders.insert(npc);
            let due = self.tick + crate::timer::TimerWheel::ms_to_ticks(COMMANDER_TALK_PAUSE_MS);
            self.timers
                .schedule_at(due, TimerKind::Fort(FortTimer::CommanderCanTalk(npc)));
        }
        attacker
    }

    fn fort_creature_name(&self, id: ObjectId) -> String {
        if let Some(p) = self.players.get(&id) {
            return p.data.name.clone();
        }
        self.npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// Java `FortCommander.doDie(killer)` after `super.doDie`: during a siege the commander is
    /// reported dead.
    pub(crate) fn fort_commander_died(&mut self, npc: ObjectId) {
        if self.npcs.get(&npc).map(|n| n.class) != Some(NpcClass::FortCommander) {
            return;
        }
        let Some(fort) = self.npc_fort(npc) else {
            return;
        };
        if self.fort_siege_in_progress(fort) {
            self.fort_killed_commander(fort, npc);
        }
    }

    /// Whether a fort-related access check passes: `hasAccess(CASTLE_SIEGE)`.
    pub(crate) fn fort_has_siege_access(&self, player: ObjectId) -> bool {
        self.has_clan_access(player, ClanAccess::CastleSiege)
    }
}
