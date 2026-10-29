//! The Hall of Erosion (defence): a command channel keeps the Seeds of Life alive for twenty
//! minutes against Ekimus' undead, for `Q00697_DefendTheHallOfErosion`.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfErosionDefence/HallOfErosionDefence.java
//
// NOTE(java-quirk): Java's `onKillByMob` and `conquestConclusion` (the failed defence) are never
// called (`onKillByMob` overrides nothing), and the script never calls
// `SeedOfInfinityManager.notifyHoEDefSuccess()`: they are not ported.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::common::{self, NpcAt, Worlds};

const INSTANCEID: i32 = 120;
const MOUTHOFEKIMUS: i32 = 32537;
const TUMOR_ALIVE: i32 = 18708;
const TUMOR_DEAD: i32 = 32535;
const SEED: i32 = 32541;

const Q00697: &str = "Q00697_DefendTheHallOfErosion";

const ENTER_TELEPORT: (i32, i32, i32) = (-179659, 211061, -12784);

const NOTMOVE: [i32; 6] = [18667, 18668, 18708, 18709, 18711, TUMOR_DEAD];

const MONSTERS: [i32; 6] = [22516, 22520, 22522, 22524, 22526, 22532];

const SEEDS_SPAWN: [[i32; 7]; 4] = [
    [SEED, -178418, 211653, -12029, 49151, 0, 1],
    [SEED, -178417, 206558, -12029, 16384, 0, 1],
    [SEED, -180911, 206551, -12029, 16384, 0, 1],
    [SEED, -180911, 211652, -12029, 49151, 0, 1],
];

const TUMOR_DEAD_SPAWN: [[i32; 7]; 4] = [
    [TUMOR_DEAD, -176036, 210002, -11948, 36863, 0, 1],
    [TUMOR_DEAD, -176039, 208203, -11948, 28672, 0, 1],
    [TUMOR_DEAD, -183288, 208205, -11948, 4096, 0, 1],
    [TUMOR_DEAD, -183290, 210004, -11948, 61439, 0, 1],
];

const TUMOR_ALIVE_SPAWN: [[i32; 7]; 4] = [
    [TUMOR_ALIVE, -176036, 210002, -11948, 36863, 0, 1],
    [TUMOR_ALIVE, -176039, 208203, -11948, 28672, 0, 1],
    [TUMOR_ALIVE, -183288, 208205, -11948, 4096, 0, 1],
    [TUMOR_ALIVE, -183290, 210004, -11948, 61439, 0, 1],
];

// @formatter:off
const ROOMS_MOBS: [[i32; 7]; 69] = [
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
    [18667, -179664, 209443, -12476, 16384, 120, 1],
    [18711, -179093, 209738, -12480, 40279, 120, 1],
    [18667, -178248, 209688, -12479, 24320, 120, 1],
    [18668, -177998, 209100, -12480, 16304, 120, 1],
    [18711, -178246, 208493, -12480, 8968, 120, 1],
    [18668, -178808, 208339, -12480, -1540, 120, 1],
    [18711, -179663, 208738, -12480, 0, 120, 1],
    [18711, -180498, 208330, -12467, 3208, 120, 1],
    [18667, -181070, 208502, -12467, -7552, 120, 1],
    [18668, -181310, 209097, -12467, -16408, 120, 1],
    [18711, -181069, 209698, -12467, -24792, 120, 1],
    [18668, -180228, 209744, -12467, 25920, 120, 1],
];
// @formatter:on

/// Java `HEDWorld`.
#[derive(Debug, Default)]
struct HedWorld {
    npc_list: Vec<NpcAt>,
    alivetumor: Vec<NpcAt>,
    dead_tumors: Vec<NpcAt>,
    dead_tumor: Option<NpcAt>,
    start_time: i64,
}

/// The script's fields and its worlds.
// NOTE(java-quirk): `conquestEnded`, `tumorRespawnTime`, `tumorKillCount` and `soulwagonSpawned`
// are fields of the script (and `seedKills` a static), shared by every instance: the Ravenous Soul
// Devourer comes once per server run, at the fourth tumor killed in any instance.
struct State {
    worlds: Worlds<HedWorld>,
    conquest_ended: bool,
    tumor_respawn_time: i64,
    tumor_kill_count: i32,
    soulwagon_spawned: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            worlds: Worlds::new(),
            conquest_ended: false,
            tumor_respawn_time: 180 * 1000,
            tumor_kill_count: 0,
            soulwagon_spawned: false,
        }
    }
}

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut State {
    ctx.state::<State>()
}

fn hed<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32) -> &'a mut HedWorld {
    state(ctx).worlds.entry(instance_id).or_default()
}

fn world_of(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let instance_id = common::world_of(ctx, object)?;
    state(ctx)
        .worlds
        .contains_key(&instance_id)
        .then_some(instance_id)
}

/// Java `ai.areas.Gracia.instances.HallOfErosionDefence.HallOfErosionDefence`.
pub struct HallOfErosionDefence;

impl HallOfErosionDefence {
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
            seeds.erosion_defence_min_players,
            seeds.erosion_defence_max_players,
            Some(Q00697),
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
            hed(ctx, instance_id).start_time = now;
            tracing::info!(
                "Hall Of Erosion Defence started {INSTANCEID} Instance: {instance_id} created by player: {}",
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

            // `finishTask = ThreadPool.schedule(new FinishTask(world), 20 * 60000)`.
            ctx.start_quest_timer(
                &format!("FinishTask {instance_id}"),
                20 * 60000,
                None,
                None,
                false,
            );
            Self::run_tumors(ctx, instance_id);
        }
    }

    fn run_tumors(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        for spawn in &ROOMS_MOBS {
            // NOTE(java-quirk): the list is made new for every row, so it keeps the last spawn only.
            let spawned = common::spawn_with_respawn(ctx, spawn, instance_id);
            hed(ctx, instance_id).npc_list = spawned;
        }

        for spawn in &TUMOR_DEAD_SPAWN {
            for _ in 0..spawn[6] {
                if let Some(npc) = common::spawn(
                    ctx,
                    spawn[0],
                    (spawn[1], spawn[2], spawn[3], spawn[4]),
                    instance_id,
                ) {
                    hed(ctx, instance_id).dead_tumors.push(npc);
                    let name = format!("RegenerationCoffinSpawn {instance_id} {}", npc.npc.object_id.0);
                    ctx.start_quest_timer(&name, 1000, None, None, false);
                }
            }
        }

        for spawn in &SEEDS_SPAWN {
            for _ in 0..spawn[6] {
                common::spawn(
                    ctx,
                    spawn[0],
                    (spawn[1], spawn[2], spawn[3], spawn[4]),
                    instance_id,
                );
            }
        }

        ctx.start_quest_timer(
            &format!("AliveTumors {instance_id}"),
            180 * 1000,
            None,
            None,
            false,
        );
    }

    fn stop_dead_tumors(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let npcs = std::mem::take(&mut hed(ctx, instance_id).dead_tumors);
        for npc in npcs {
            ctx.delete_npc(npc.npc);
        }
    }

    fn notify_tumor_death(ctx: &mut ScriptCtx<'_>, at: Option<(i32, i32, i32, i32)>, instance_id: i32) {
        let s = state(ctx);
        s.tumor_kill_count += 1;
        if s.tumor_kill_count == 4 && !s.soulwagon_spawned {
            s.soulwagon_spawned = true;
            if let Some(at) = at
                && let Some(soul) = common::spawn(ctx, 25636, at, instance_id)
            {
                ctx.broadcast_npc_say(soul.npc, ChatType::Shout, NpcStringId::HaHaHa, &[]);
            }
        }
    }

    /// Java `FinishTask.run()`: the defence held.
    fn finish_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        state(ctx).conquest_ended = true;
        if ctx.world().instance(instance_id).is_none() {
            return;
        }
        for player in common::allowed(ctx, instance_id) {
            if let Some(qs) = ctx.quest_state_of(Q00697, player, false)
                && qs.is_cond(ctx, 1)
            {
                qs.set(ctx, "defenceDone", "1");
            }
        }

        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::CongratulationsYouHaveSucceededAtS1S2TheInstanceWillShortlyExpire,
            &[],
        );
        common::conclude(ctx, instance_id);
    }

    /// The timers that are Java's `ThreadPool.schedule` tasks.
    fn on_task(ctx: &mut ScriptCtx<'_>, event: &str) -> bool {
        let mut parts = event.split(' ');
        let task = parts.next().unwrap_or_default();
        let Some(instance_id) = parts.next().and_then(|v| v.parse::<i32>().ok()) else {
            return false;
        };
        let arg = parts.next().and_then(|v| v.parse::<i32>().ok());
        match task {
            "FinishTask" => Self::finish_task(ctx, instance_id),
            "AliveTumors" => {
                if !state(ctx).conquest_ended {
                    Self::stop_dead_tumors(ctx, instance_id);
                    for spawn in &TUMOR_ALIVE_SPAWN {
                        for _ in 0..spawn[6] {
                            if let Some(npc) = common::spawn(
                                ctx,
                                spawn[0],
                                (spawn[1], spawn[2], spawn[3], spawn[4]),
                                instance_id,
                            ) {
                                hed(ctx, instance_id).alivetumor.push(npc);
                            }
                        }
                    }

                    common::screen_message(
                        ctx,
                        instance_id,
                        NpcStringId::TheTumorInsideS1HasCompletelyRevivedNrecoveredNearbyUndeadAreSwarmingTowardSeedOfLife,
                        &[],
                    );
                }
            }
            "RegenerationCoffinSpawn" => {
                if state(ctx).conquest_ended {
                    return true;
                }
                let Some(npc) = arg.and_then(|id| {
                    state(ctx)
                        .worlds
                        .get(&instance_id)
                        .and_then(|w| w.dead_tumors.iter().find(|t| t.npc.object_id.0 == id).copied())
                }) else {
                    return true;
                };
                let loc = npc.location(ctx);
                for _ in 0..4 {
                    if let Some(worm) = common::spawn(ctx, 18709, loc, instance_id) {
                        hed(ctx, instance_id).dead_tumors.push(worm);
                    }
                }
            }
            "TumorRevival" => {
                // NOTE(java-quirk): the task takes `world.deadTumor` when it runs, the last
                // tumor that died in the instance, not the one whose death started it.
                let Some(dead_tumor) = state(ctx).worlds.get(&instance_id).and_then(|w| w.dead_tumor) else {
                    return true;
                };
                let loc = dead_tumor.location(ctx);
                ctx.delete_npc(dead_tumor.npc);
                if let Some(tumor) = common::spawn(ctx, TUMOR_ALIVE, loc, instance_id) {
                    hed(ctx, instance_id).alivetumor.push(tumor);
                }
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::TheTumorInsideS1HasCompletelyRevivedNrecoveredNearbyUndeadAreSwarmingTowardSeedOfLife,
                    &[],
                );
            }
            _ => return false,
        }
        true
    }
}

impl Script for HallOfErosionDefence {
    fn name(&self) -> &'static str {
        "HallOfErosionDefence"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/instances/HallOfErosionDefence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOUTHOFEKIMUS]);
        s.add_talk_id(&[MOUTHOFEKIMUS]);
        s.add_start_npc(&[TUMOR_DEAD]);
        s.add_talk_id(&[TUMOR_DEAD]);

        s.add_spawn_id(&NOTMOVE);
        s.add_spawn_id(&[SEED]);

        s.add_aggro_range_enter_id(&[18668]);

        s.add_kill_id(&[TUMOR_ALIVE]);
        s.add_kill_id(&[SEED]);
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
                let world = hed(ctx, instance_id);
                (std::mem::take(&mut world.dead_tumors), world.dead_tumor)
            };
            common::warp(ctx, instance_id, player, &mut dead_tumors, dead_tumor);
            hed(ctx, instance_id).dead_tumors = dead_tumors;
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
            common::ward_of_death(ctx, npc, instance_id, &MONSTERS, false);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if NOTMOVE.contains(&npc.id) {
            ctx.set_random_walking(npc, false);
            ctx.set_immobilized(npc, true);
        }

        if npc.id == SEED {
            // NOTE(java-quirk): `((QuestGuard) npc).setPassive(true)`: the Unstable Seed of
            // Generation is a `Folk`, the cast throws and the rest of `onSpawn` does not run (it
            // does nothing for the seed anyway).
            return;
        }

        if let Some(instance_id) = world_of(ctx, npc.object_id)
            && npc.id == TUMOR_DEAD
        {
            common::add_tag(ctx, instance_id, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        if npc.id == TUMOR_ALIVE {
            let loc = common::location_of(ctx, npc.object_id);
            let count = i64::from(ctx.get_random_range(2, 5));
            ctx.drop_item(npc, Some(player), 13797, count);
            ctx.delete_npc(npc);
            Self::notify_tumor_death(ctx, loc, instance_id);
            let dead_tumor = loc.and_then(|loc| common::spawn(ctx, TUMOR_DEAD, loc, instance_id));
            {
                let world = hed(ctx, instance_id);
                world.dead_tumor = dead_tumor;
                if let Some(dead) = dead_tumor {
                    world.dead_tumors.push(dead);
                }
            }
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::TheTumorInsideS1HasBeenDestroyedNtheNearbyUndeadThatWereAttackingSeedOfLifeStartLosingTheirEnergyAndRunAway,
                &[],
            );
            let respawn = state(ctx).tumor_respawn_time;
            let name = format!("TumorRevival {instance_id} {}", npc.object_id.0);
            ctx.start_quest_timer(&name, respawn, None, None, false);
        }

        if npc.id == 18711 {
            state(ctx).tumor_respawn_time += 5 * 1000;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, QuestWrite, ScriptRegistry};

    use super::*;
    use crate::ai::areas::gracia::testing::{STRATEGY_GUIDE, command_channel};
    use crate::testing::{ScriptTest, item, player_data};

    const NPCS: &str = r#"
        <npc id="32537" type="Folk" level="70" name="Mouth of Ekimus"/>
        <npc id="32535" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="32541" type="Folk" level="70" name="Unstable Seed of Generation"/>
        <npc id="18708" type="Monster" level="81" name="Tumor of Death"/>
        <npc id="18709" type="Monster" level="80" name="Soul Coffin"/>
        <npc id="25636" type="Monster" level="81" name="Ravenous Soul Devourer"/>"#;
    const ITEMS: &str = r#"<item id="8871" type="EtcItem" name="Strategy Guide"/>
        <item id="13797" type="EtcItem" name="Soul Stone Fragment"><set name="is_stackable" val="true"/></item>"#;

    struct Quest697;

    impl Script for Quest697 {
        fn name(&self) -> &'static str {
            Q00697
        }
        fn path(&self) -> &'static str {
            "quests/Q00697_DefendTheHallOfErosion"
        }
        fn id(&self) -> i32 {
            697
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        t.npcs_of(npc_id)
            .into_iter()
            .filter(|n| !t.world.is_dead(*n))
            .collect()
    }

    #[test]
    fn the_seeds_hold_twenty_minutes_and_the_defence_is_done() {
        let mut registry = ScriptRegistry::new();
        registry.add(HallOfErosionDefence);
        registry.add(Quest697);
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &[], |world, _| {
            world.set_script_settings(l2_world::ScriptSettings {
                gracia_seeds: l2_world::script::GraciaSeedsSettings {
                    erosion_defence_min_players: 2,
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
                states: vec![(Q00697.to_owned(), Some("Started".to_owned()))],
                variables: vec![(Q00697.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
            };
            t.enter_with(data);
        }
        command_channel(&mut t, &[&[1, 2], &[3, 4]]);
        let mouth = ScriptNpc {
            object_id: t.spawn(MOUTHOFEKIMUS, 20),
            id: MOUTHOFEKIMUS,
        };
        t.world.run_as_script("HallOfErosionDefence", |ctx| {
            HallOfErosionDefence.on_talk(ctx, mouth, Oid(1));
        });
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_world(instance).unwrap().allowed.len(), 4);
        // Four dead tumors and the four seeds; a second later four coffins at each tumor.
        assert_eq!(t.npcs_of(TUMOR_DEAD).len(), 4);
        assert_eq!(t.npcs_of(SEED).len(), 4);
        t.wait_ms(1_100);
        assert_eq!(alive(&t, 18709).len(), 16);
        // Three minutes: the tumors come alive.
        t.wait_ms(180_000);
        assert!(t.npcs_of(TUMOR_DEAD).is_empty());
        let tumors = alive(&t, TUMOR_ALIVE);
        assert_eq!(tumors.len(), 4);
        // The fourth tumor killed calls the Ravenous Soul Devourer.
        for tumor in tumors {
            t.kill(1, tumor);
        }
        assert_eq!(t.npcs_of(25636).len(), 1);
        assert_eq!(t.npcs_of(TUMOR_DEAD).len(), 4);
        // Twenty minutes after the start the defence held.
        t.take_writes();
        t.wait_ms(20 * 60_000 - 190_000);
        let writes = t.take_writes();
        assert!(
            writes.iter().any(|w| matches!(w,
                QuestWrite::InsertVar { char_id: 3, var, value, .. } if var == "defenceDone" && value == "1")),
            "{writes:?}"
        );
        assert!(t.npcs_of(SEED).is_empty());
    }
}
