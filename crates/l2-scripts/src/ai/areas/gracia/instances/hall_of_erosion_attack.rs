//! The Hall of Erosion (attack): a command channel destroys the Tumors of Death until Cohemenes
//! shows himself, and kills him for `Q00696_ConquerTheHallOfErosion` and the Seed of Infinity.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfErosionAttack/HallOfErosionAttack.java

use l2_protocol_game::server::{ChatType, SystemMessage, SystemMessageParam};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_util::time::local_offset_millis;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::common::{self, NpcAt, Worlds};
use crate::ai::areas::gracia::seeds::{SOI_MANAGER, SOI_NOTIFY_COHEMENES_KILL};

const INSTANCEID: i32 = 119;
const INSTANCEPENALTY: i64 = 24;
const MOUTHOFEKIMUS: i32 = 32537;
const TUMOR_ALIVE: i32 = 18708;
const TUMOR_DEAD: i32 = 32535;
const COHEMENES: i32 = 25634;

const Q00696: &str = "Q00696_ConquerTheHallOfErosion";

const ENTER_TELEPORT: (i32, i32, i32) = (-179659, 211061, -12784);

const NOTMOVE: [i32; 4] = [18668, 18711, 18708, TUMOR_DEAD];

const MONSTERS: [i32; 4] = [22516, 22520, 22522, 22524];

// @formatter:off
const ROOMS_MOBS: [[i32; 7]; 57] = [
    [22516, -180364, 211944, -12019, 0, 60, 1],
    [22516, -181616, 211413, -12015, 0, 60, 1],
    [22520, -181404, 211042, -12023, 0, 60, 1],
    [22522, -181558, 212227, -12035, 0, 60, 1],
    [22522, -180459, 212322, -12018, 0, 60, 1],
    [22524, -180428, 211180, -12014, 0, 60, 1],
    [22524, -180718, 212162, -12028, 0, 60, 1],
    [22532, -183114, 209397, -11923, 0, 60, 1],
    [22532, -182917, 210495, -11925, 0, 60, 1],
    [22516, -183918, 210225, -11934, 0, 60, 1],
    [22532, -183862, 209909, -11932, 0, 60, 1],
    [22532, -183246, 210631, -11923, 0, 60, 1],
    [22522, -182971, 210522, -11924, 0, 60, 1],
    [22522, -183485, 209406, -11921, 0, 60, 1],
    [22516, -183032, 208822, -11923, 0, 60, 1],
    [22516, -182709, 207817, -11929, 0, 60, 1],
    [22520, -182964, 207746, -11924, 0, 60, 1],
    [22520, -183385, 208847, -11922, 0, 60, 1],
    [22526, -183684, 208847, -11926, 0, 60, 1],
    [22526, -183530, 208725, -11926, 0, 60, 1],
    [22532, -183968, 207603, -11928, 0, 60, 1],
    [22532, -183608, 208567, -11926, 0, 60, 1],
    [22526, -181471, 207159, -12020, 0, 60, 1],
    [22526, -180213, 207042, -12013, 0, 60, 1],
    [22532, -180213, 206506, -12010, 0, 60, 1],
    [22532, -181720, 206643, -12016, 0, 60, 1],
    [22516, -181743, 206643, -12018, 0, 60, 1],
    [22516, -181028, 205739, -12030, 0, 60, 1],
    [22520, -181431, 205980, -12040, 0, 60, 1],
    [22524, -178964, 207168, -12014, 0, 60, 1],
    [22524, -177658, 207037, -12019, 0, 60, 1],
    [22522, -177730, 206558, -12016, 0, 60, 1],
    [22522, -179132, 206650, -12011, 0, 60, 1],
    [22526, -179132, 206155, -12017, 0, 60, 1],
    [22526, -178277, 205754, -12031, 0, 60, 1],
    [22516, -178716, 205802, -12020, 0, 60, 1],
    [22532, -176565, 207839, -11929, 0, 60, 1],
    [22532, -176281, 208822, -11923, 0, 60, 1],
    [22520, -175791, 208804, -11923, 0, 60, 1],
    [22520, -176259, 207689, -11923, 0, 60, 1],
    [22526, -175849, 207508, -11929, 0, 60, 1],
    [22526, -175453, 208250, -11930, 0, 60, 1],
    [22524, -175738, 207914, -11946, 0, 60, 1],
    [22526, -176339, 209425, -11923, 0, 60, 1],
    [22526, -176586, 210424, -11928, 0, 60, 1],
    [22516, -176586, 210546, -11923, 0, 60, 1],
    [22516, -175847, 209365, -11922, 0, 60, 1],
    [22520, -175496, 209498, -11924, 0, 60, 1],
    [22520, -175538, 210252, -11940, 0, 60, 1],
    [22524, -175527, 209744, -11928, 0, 60, 1],
    [22520, -177940, 210876, -12005, 0, 60, 1],
    [22520, -178935, 210903, -12018, 0, 60, 1],
    [22522, -179331, 211365, -12013, 0, 60, 1],
    [22522, -177637, 211579, -12015, 0, 60, 1],
    [22526, -177837, 212356, -12037, 0, 60, 1],
    [22526, -179030, 212261, -12018, 0, 60, 1],
    [22532, -178367, 212328, -12031, 0, 60, 1],
];

const ROOMS_TUMORS: [[i32; 7]; 20] = [
    [18780, -180911, 211652, -12029, 49151, 240, 1],
    [18780, -180911, 206551, -12029, 16384, 240, 1],
    [18780, -178417, 206558, -12032, 16384, 240, 1],
    [18780, -178418, 211653, -12029, 49151, 240, 1],
    [18708, -183290, 210004, -11948, 61439, 0, 1],
    [18708, -183288, 208205, -11948, 4096, 0, 1],
    [18708, -176039, 208203, -11948, 28672, 0, 1],
    [18708, -176036, 210002, -11948, 36863, 0, 1],
    [18668, -179664, 209443, -12476, 16384, 120, 1],
    [18668, -179093, 209738, -12480, 40279, 120, 1],
    [18668, -178248, 209688, -12479, 24320, 120, 1],
    [18668, -177998, 209100, -12480, 16304, 120, 1],
    [18668, -178246, 208493, -12480, 8968, 120, 1],
    [18668, -178808, 208339, -12480, -1540, 120, 1],
    [18668, -179663, 208738, -12480, 0, 120, 1],
    [18668, -180498, 208330, -12467, 3208, 120, 1],
    [18668, -181070, 208502, -12467, -7552, 120, 1],
    [18668, -181310, 209097, -12467, -16408, 120, 1],
    [18668, -181069, 209698, -12467, -24792, 120, 1],
    [18668, -180228, 209744, -12467, 25920, 120, 1],
];

const COHEMENES_SPAWN: [[i32; 7]; 4] = [
    [25634, -178472, 211823, -12025, 0, 0, -1],
    [25634, -180926, 211887, -12029, 0, 0, -1],
    [25634, -180906, 206635, -12032, 0, 0, -1],
    [25634, -178492, 206426, -12023, 0, 0, -1],
];
// @formatter:on

/// Java `HEAWorld`.
#[derive(Debug, Default)]
struct HeaWorld {
    npc_list: Vec<NpcAt>,
    dead_tumors: Vec<NpcAt>,
    tumor_count: i32,
    cohemenes: Option<ScriptNpc>,
    dead_tumor: Option<NpcAt>,
    is_boss_attacked: bool,
    start_time: i64,
}

/// The script's fields and its worlds.
// NOTE(java-quirk): `conquestEnded` and `tumorRespawnTime` are fields of the script, shared by
// every instance: one conquest that ends stops the tumors of all, and every Soul Coffin (18711)
// killed anywhere makes the tumors of all come back later.
struct State {
    worlds: Worlds<HeaWorld>,
    conquest_ended: bool,
    tumor_respawn_time: i64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            worlds: Worlds::new(),
            conquest_ended: false,
            tumor_respawn_time: 180 * 1000,
        }
    }
}

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut State {
    ctx.state::<State>()
}

fn hea<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32) -> &'a mut HeaWorld {
    state(ctx).worlds.entry(instance_id).or_default()
}

/// `InstanceManager.getWorld(object) instanceof HEAWorld`.
fn world_of(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let instance_id = common::world_of(ctx, object)?;
    state(ctx)
        .worlds
        .contains_key(&instance_id)
        .then_some(instance_id)
}

/// Java `ai.areas.Gracia.instances.HallOfErosionAttack.HallOfErosionAttack`.
pub struct HallOfErosionAttack;

impl HallOfErosionAttack {
    fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
        if ctx.is_gm(player) {
            return true;
        }

        if ctx.world().party_of(player).is_none() {
            ctx.send_system_message(
                player,
                SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter,
            );
            return false;
        }

        if !common::is_party_leader(ctx, player) {
            ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            return false;
        }

        if ctx.world().command_channel_of(player).is_none() {
            ctx.send_system_message(
                player,
                SystemMessageId::YouCannotEnterBecauseYouAreNotAssociatedWithTheCurrentCommandChannel,
            );
            return false;
        }

        if !common::is_channel_leader(ctx, player) {
            ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            return false;
        }

        let seeds = ctx.script_settings().gracia_seeds.clone();
        common::check_channel_members(
            ctx,
            player,
            INSTANCEID,
            seeds.erosion_attack_min_players,
            seeds.erosion_attack_max_players,
            Some(Q00696),
        )
    }

    fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if let Some(instance_id) = common::player_world(ctx, player) {
            if !state(ctx).worlds.contains_key(&instance_id) {
                ctx.send_system_message(
                    player,
                    SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                );
                return;
            }

            common::teleport_into(ctx, player, ENTER_TELEPORT, instance_id);
            return;
        }

        if Self::check_conditions(ctx, player) {
            let instance_id = common::create_world(ctx, INSTANCEID);
            let now = ctx.now_ms();
            hea(ctx, instance_id).start_time = now;
            tracing::info!(
                "Hall Of Erosion Attack started {INSTANCEID} Instance: {instance_id} created by player: {}",
                ctx.player_name(player)
            );
            if ctx.world().party_of(player).is_some() {
                let members = if ctx.world().is_in_command_channel(player) {
                    common::channel_members(ctx, player)
                } else {
                    ctx.world().party_members_of(player)
                };
                for party_member in members {
                    common::teleport_into(ctx, party_member, ENTER_TELEPORT, instance_id);
                    common::add_allowed(ctx, instance_id, party_member);
                }
            } else {
                common::teleport_into(ctx, player, ENTER_TELEPORT, instance_id);
                common::add_allowed(ctx, instance_id, player);
            }

            Self::run_tumors(ctx, instance_id);
        }
    }

    fn run_tumors(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        for spawn in &ROOMS_MOBS {
            common::spawn_with_respawn(ctx, spawn, instance_id);
        }

        for spawn in &ROOMS_TUMORS {
            let spawned = common::spawn_with_respawn(ctx, spawn, instance_id);
            hea(ctx, instance_id).npc_list.extend(spawned);
        }

        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::YouCanHearTheUndeadOfEkimusRushingTowardYouS1S2ItHasNowBegun,
            &[],
        );
    }

    fn stop_tumors(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let npcs = std::mem::take(&mut hea(ctx, instance_id).npc_list);
        for npc in npcs {
            ctx.delete_npc(npc.npc);
        }
    }

    /// Java `finishInstance(world)`: the entry opens again at the next 06:30.
    fn finish_instance(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let now = ctx.now_ms();
        let reenter = crate::instances::kamaloka_reset_time(now, local_offset_millis(now), false);

        let sm = SystemMessage::new(
            SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
        )
        .with(SystemMessageParam::InstanceName(INSTANCEID));

        for plr in common::allowed(ctx, instance_id) {
            ctx.world_mut().set_instance_time(plr, INSTANCEID, reenter);
            if common::is_online(ctx, plr) {
                ctx.world().send(plr, &sm);
            }
        }

        common::expire_in_five_minutes(ctx, instance_id);
    }

    /// The timers that are Java's `ThreadPool.schedule` tasks: `TumorRevival` and
    /// `RegenerationCoffinSpawn` of a dead tumor.
    fn on_task(ctx: &mut ScriptCtx<'_>, event: &str) -> bool {
        let mut parts = event.split(' ');
        let (Some(task), Some(instance_id), Some(tumor)) = (
            parts.next(),
            parts.next().and_then(|v| v.parse::<i32>().ok()),
            parts.next().and_then(|v| v.parse::<i32>().ok()),
        ) else {
            return false;
        };
        if task != "TumorRevival" && task != "RegenerationCoffinSpawn" {
            return false;
        }
        if state(ctx).conquest_ended {
            return true;
        }
        let Some(dead_tumor) = state(ctx)
            .worlds
            .get(&instance_id)
            .and_then(|w| w.dead_tumors.iter().find(|t| t.npc.object_id.0 == tumor).copied())
        else {
            return true;
        };
        let loc = dead_tumor.location(ctx);
        if task == "TumorRevival" {
            if let Some(tumor) = common::spawn(ctx, TUMOR_ALIVE, loc, instance_id) {
                hea(ctx, instance_id).npc_list.push(tumor);
            }
            ctx.delete_npc(dead_tumor.npc);
            common::add_tag(ctx, instance_id, -1);
        } else {
            for _ in 0..4 {
                if let Some(worm) = common::spawn(ctx, 18710, loc, instance_id) {
                    hea(ctx, instance_id).npc_list.push(worm);
                }
            }
        }
        true
    }
}

impl Script for HallOfErosionAttack {
    fn name(&self) -> &'static str {
        "HallOfErosionAttack"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/instances/HallOfErosionAttack"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOUTHOFEKIMUS]);
        s.add_talk_id(&[MOUTHOFEKIMUS]);
        s.add_start_npc(&[TUMOR_DEAD]);
        s.add_talk_id(&[TUMOR_DEAD]);

        s.add_spawn_id(&[COHEMENES]);
        s.add_spawn_id(&NOTMOVE);

        s.add_aggro_range_enter_id(&[18668]);

        s.add_attack_id(&[COHEMENES]);

        s.add_kill_id(&[TUMOR_ALIVE]);
        s.add_kill_id(&[COHEMENES]);
        s.add_kill_id(&[18711]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if Self::on_task(ctx, event) {
            return None;
        }
        let player = player?;
        let instance_id = common::player_world(ctx, player)?;
        if !state(ctx).worlds.contains_key(&instance_id) {
            return None;
        }
        if event.starts_with("warp") {
            let (mut dead_tumors, dead_tumor) = {
                let world = hea(ctx, instance_id);
                (std::mem::take(&mut world.dead_tumors), world.dead_tumor)
            };
            common::warp(ctx, instance_id, player, &mut dead_tumors, dead_tumor);
            hea(ctx, instance_id).dead_tumors = dead_tumors;
        }

        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == MOUTHOFEKIMUS {
            Self::enter_instance(ctx, player);
        }

        None
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: ObjectId,
        _is_summon: bool,
    ) {
        if let Some(instance_id) = world_of(ctx, npc.object_id)
            && npc.id == 18668
        {
            common::ward_of_death(ctx, npc, instance_id, &MONSTERS, true);
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        if !hea(ctx, instance_id).is_boss_attacked {
            hea(ctx, instance_id).is_boss_attacked = true;
            common::restrict_entry(ctx, instance_id, INSTANCEID, INSTANCEPENALTY, true);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if NOTMOVE.contains(&npc.id) {
            ctx.set_random_walking(npc, false);
            ctx.set_immobilized(npc, true);
        }

        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        if npc.id == TUMOR_ALIVE {
            let world = hea(ctx, instance_id);
            world.tumor_count += 1;
            if world.tumor_count == 4
                && let Some(cohemenes) = world.cohemenes
            {
                ctx.delete_npc(cohemenes);
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::YouHaveFailedAtS1S2TheInstanceWillShortlyExpire,
                    &[],
                );
                Self::finish_instance(ctx, instance_id);
                state(ctx).conquest_ended = true;
                Self::stop_tumors(ctx, instance_id);
            }
        }

        if npc.id == TUMOR_DEAD {
            common::add_tag(ctx, instance_id, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        let loc = common::location_of(ctx, npc.object_id);
        if npc.id == TUMOR_ALIVE {
            hea(ctx, instance_id).tumor_count -= 1;
            let count = i64::from(ctx.get_random_range(2, 5));
            ctx.drop_item(npc, Some(player), 13797, count);
            ctx.delete_npc(npc);
            let dead_tumor = loc.and_then(|loc| common::spawn(ctx, TUMOR_DEAD, loc, instance_id));
            {
                let world = hea(ctx, instance_id);
                world.dead_tumor = dead_tumor;
                if let Some(dead) = dead_tumor {
                    world.dead_tumors.push(dead);
                }
            }
            if let Some(dead) = dead_tumor {
                let respawn = state(ctx).tumor_respawn_time;
                let id = dead.npc.object_id.0;
                ctx.start_quest_timer(
                    &format!("TumorRevival {instance_id} {id}"),
                    respawn,
                    None,
                    None,
                    false,
                );
                ctx.start_quest_timer(
                    &format!("RegenerationCoffinSpawn {instance_id} {id}"),
                    20000,
                    None,
                    None,
                    false,
                );
            }
            let (tumor_count, cohemenes) = {
                let world = hea(ctx, instance_id);
                (world.tumor_count, world.cohemenes)
            };
            if tumor_count >= 1 {
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::TheTumorInsideS1HasBeenDestroyedNinOrderToDrawOutTheCowardlyCohemenesYouMustDestroyAllTheTumors,
                    &[],
                );
            }

            if tumor_count == 0 && cohemenes.is_none() {
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::AllTheTumorsInsideS1HaveBeenDestroyedDrivenIntoACornerCohemenesAppearsCloseBy,
                    &[],
                );
                let index = ctx.get_random_range(0, COHEMENES_SPAWN.len() as i32 - 1);
                let spawn = COHEMENES_SPAWN[usize::try_from(index).unwrap_or(0)];
                if let Some(n) = ctx.add_spawn(
                    spawn[0],
                    spawn[1],
                    spawn[2],
                    spawn[3],
                    spawn[4],
                    false,
                    0,
                    instance_id,
                ) {
                    ctx.broadcast_npc_say(
                        n,
                        ChatType::Shout,
                        NpcStringId::CMonCMonShowYourFaceYouLittleRatsLetMeSeeWhatTheDoomedWeaklingsAreScheming,
                        &[],
                    );
                    hea(ctx, instance_id).cohemenes = Some(n);
                }
            }
        }

        if npc.id == COHEMENES {
            ctx.broadcast_npc_say(
                npc,
                ChatType::Shout,
                NpcStringId::KeuIWillLeaveForNowButDonTThinkThisIsOverTheSeedOfInfinityCanNeverDie,
                &[],
            );
            for plr in common::allowed(ctx, instance_id) {
                if let Some(qs) = ctx.quest_state_of(Q00696, plr, false)
                    && qs.is_cond(ctx, 1)
                {
                    qs.set(ctx, "cohemenes", "1");
                }
            }

            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::CongratulationsYouHaveSucceededAtS1S2TheInstanceWillShortlyExpire,
                &[],
            );
            hea(ctx, instance_id).cohemenes = None;
            state(ctx).conquest_ended = true;
            Self::finish_instance(ctx, instance_id);
            Self::stop_tumors(ctx, instance_id);
            ctx.notify_event_of(SOI_MANAGER, SOI_NOTIFY_COHEMENES_KILL, None, None);
        }

        if npc.id == 18711 {
            state(ctx).tumor_respawn_time += 10 * 1000;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, QuestWrite, ScriptRegistry};

    use super::*;
    use crate::ai::areas::gracia::seeds::SeedOfInfinityManager;
    use crate::ai::areas::gracia::testing::{STRATEGY_GUIDE, command_channel};
    use crate::testing::{ScriptTest, item, player_data};

    const NPCS: &str = r#"
        <npc id="32537" type="Folk" level="70" name="Mouth of Ekimus"/>
        <npc id="32535" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="18708" type="Monster" level="81" name="Tumor of Death"/>
        <npc id="18780" type="Monster" level="83" name="Symbol of Cohemenes"/>
        <npc id="18668" type="Monster" level="83" name="Ward of Death"/>
        <npc id="18710" type="Monster" level="81" name="Soul Coffin"/>
        <npc id="25634" type="Monster" level="81" name="Cohemenes"/>
        <npc id="22516" type="Monster" level="80" name="Mob"/>
        <npc id="22520" type="Monster" level="80" name="Mob"/>
        <npc id="22522" type="Monster" level="80" name="Mob"/>
        <npc id="22524" type="Monster" level="80" name="Mob"/>
        <npc id="22526" type="Monster" level="80" name="Mob"/>
        <npc id="22532" type="Monster" level="80" name="Mob"/>"#;
    const ITEMS: &str = r#"<item id="8871" type="EtcItem" name="Strategy Guide"/>
        <item id="13797" type="EtcItem" name="Soul Stone Fragment"><set name="is_stackable" val="true"/></item>"#;

    struct Quest696;

    impl Script for Quest696 {
        fn name(&self) -> &'static str {
            Q00696
        }
        fn path(&self) -> &'static str {
            "quests/Q00696_ConquerTheHallOfErosion"
        }
        fn id(&self) -> i32 {
            696
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfInfinityManager);
        registry.add(HallOfErosionAttack);
        registry.add(Quest696);
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &[], |world, _| {
            world.set_script_settings(l2_world::ScriptSettings {
                cycle_database_writes: true,
                gracia_seeds: l2_world::script::GraciaSeedsSettings {
                    erosion_attack_min_players: 2,
                    ..Default::default()
                },
                ..Default::default()
            });
        });
        for id in 1..=4 {
            let items = if id == 1 {
                vec![item(1000, STRATEGY_GUIDE, 1)]
            } else {
                vec![]
            };
            let mut data = player_data(id, 80, items);
            data.profile.quests = QuestLoad {
                states: vec![(Q00696.to_owned(), Some("Started".to_owned()))],
                variables: vec![(Q00696.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
            };
            t.enter_with(data);
        }
        command_channel(&mut t, &[&[1, 2], &[3, 4]]);
        let mouth = ScriptNpc {
            object_id: t.spawn(MOUTHOFEKIMUS, 20),
            id: MOUTHOFEKIMUS,
        };
        (t, mouth)
    }

    fn talk(t: &mut ScriptTest, player: i32, mouth: ScriptNpc) {
        t.world.run_as_script("HallOfErosionAttack", |ctx| {
            HallOfErosionAttack.on_talk(ctx, mouth, Oid(player));
        });
    }

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        t.npcs_of(npc_id)
            .into_iter()
            .filter(|n| !t.world.is_dead(*n))
            .collect()
    }

    #[test]
    fn the_channel_enters_destroys_the_tumors_and_kills_cohemenes() {
        let (mut t, mouth) = setup();
        // Only the channel leader may ask.
        talk(&mut t, 3, mouth);
        assert_eq!(t.world.instance_id_of(Oid(3)), 0);
        talk(&mut t, 1, mouth);
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        for id in 2..=4 {
            assert_eq!(t.world.instance_id_of(Oid(id)), instance);
        }
        assert_eq!(t.world.instance_world(instance).unwrap().template_id, INSTANCEID);
        let tumors = alive(&t, TUMOR_ALIVE);
        assert_eq!(tumors.len(), 4);
        t.ticks(2);

        // A destroyed tumor drops fragments and leaves a dead tumor (the tag counts it).
        for tumor in tumors {
            t.kill(1, tumor);
        }
        assert_eq!(t.npcs_of(TUMOR_DEAD).len(), 4);
        assert_eq!(
            t.world
                .instance_world(instance)
                .unwrap()
                .parameters
                .get("tag")
                .map(String::as_str),
            Some("3")
        );
        // All four down: Cohemenes comes.
        let cohemenes = alive(&t, COHEMENES);
        assert_eq!(cohemenes.len(), 1);
        // The first hit on him restricts the entry for a day.
        let boss = ScriptNpc {
            object_id: cohemenes[0],
            id: COHEMENES,
        };
        t.world.run_as_script("HallOfErosionAttack", |ctx| {
            HallOfErosionAttack.on_attack(ctx, boss, Oid(1), 1, false, None);
        });
        assert!(t.world.instance_time(Oid(4), INSTANCEID) > 0);

        t.take_writes();
        t.kill(1, cohemenes[0]);
        // Every member's Q00696 is marked; the seed counts the kill; the instance ends.
        let writes = t.take_writes();
        for id in 1..=4 {
            assert!(
                writes.iter().any(|w| matches!(w,
                    QuestWrite::InsertVar { char_id, var, value, .. } if *char_id == id && var == "cohemenes" && value == "1")),
                "{writes:?}"
            );
        }
        assert_eq!(t.world.global_variables().get_int("SoI_CohemenesCount", 0), 1);
        // The tumors stopped: their revival does nothing any more.
        t.wait_ms(181_000);
        assert!(alive(&t, TUMOR_ALIVE).is_empty());
    }

    #[test]
    fn a_dead_tumor_revives_after_three_minutes() {
        let (mut t, mouth) = setup();
        talk(&mut t, 1, mouth);
        let tumor = alive(&t, TUMOR_ALIVE)[0];
        t.ticks(2);
        t.kill(1, tumor);
        assert_eq!(alive(&t, TUMOR_ALIVE).len(), 3);
        // Twenty seconds: four Soul Coffins at the dead tumor.
        t.wait_ms(20_000);
        assert_eq!(alive(&t, 18710).len(), 4);
        t.wait_ms(160_000);
        assert_eq!(alive(&t, TUMOR_ALIVE).len(), 4);
        assert!(t.npcs_of(TUMOR_DEAD).is_empty());
    }
}
