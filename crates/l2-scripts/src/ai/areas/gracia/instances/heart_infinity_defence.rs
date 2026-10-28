//! The Heart of Infinity (defence): a command channel keeps Ekimus from waking for thirty
//! minutes while Soul Wagons carry souls to him.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HeartInfinityDefence/HeartInfinityDefence.java
//
// NOT FIRED: `onEnterZone` (zone 200032) counts the Soul Wagons that reach Ekimus. The zone is
// in no datapack file (Java's `addEnterZoneId(200032)` listens to nothing either), and the
// world does not track NPCs in zones; the hook is ported as Java has it.
//
// NOTE(java-quirk): the success marks `Q00697_DefendTheHallOfErosion` (`defenceDone`), the quest
// of the Hall of Erosion.

use l2_protocol_game::server::ChatType;
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::common::{self, NpcAt, Worlds};
use super::heart_infinity_attack::{ROOMS_BOSSES, ROOMS_MOBS, ROOMS_TUMORS as ROOMS_ALIVE_TUMORS};

const QN: &str = "HeartInfinityDefence";
const INSTANCEID: i32 = 122;

const ABYSSGAZE: i32 = 32539;
const ALIVETUMOR: i32 = 18708;
const DEADTUMOR: i32 = 32535;
const SOULWAGON: i32 = 22523;
const REGENERATION_COFFIN: i32 = 18709;
const MAX_COFFINS: i32 = 20;

const Q00697: &str = "Q00697_DefendTheHallOfErosion";

const ENTER_TELEPORT: (i32, i32, i32) = (-179284, 205990, -15520);

const NOTMOVE: [i32; 4] = [18668, 18711, ALIVETUMOR, DEADTUMOR];

const MONSTERS: [i32; 4] = [22516, 22520, 22522, 22524];

// @formatter:off
const ROOMS_TUMORS: [[i32; 7]; 6] = [
    [32535, -179779, 212540, -15520, 49151, 0, 1],
    [32535, -177028, 211135, -15520, 36863, 0, 1],
    [32535, -176355, 208043, -15520, 28672, 0, 1],
    [32535, -179284, 205990, -15520, 16384, 0, 1],
    [32535, -182268, 208218, -15520, 4096, 0, 1],
    [32535, -182069, 211140, -15520, 61439, 0, 1],
];
// @formatter:on

/// Java `HIDWorld`.
#[derive(Debug, Default)]
struct HidWorld {
    npc_list: Vec<NpcAt>,
    dead_tumors: Vec<NpcAt>,
    dead_tumor: Option<NpcAt>,
    start_time: i64,
    /// The three tasks are scheduled and not cancelled.
    finish_task: bool,
    timer_task: bool,
    wagon_spawn_task: bool,
}

/// The script's fields and its worlds.
// NOTE(java-quirk): `preawakenedEchmus`, `coffinsCreated`, `tumorRespawnTime`,
// `wagonRespawnTime` and `conquestEnded` are fields of the script, shared by every instance.
struct State {
    worlds: Worlds<HidWorld>,
    preawakened_echmus: Option<ScriptNpc>,
    coffins_created: i32,
    tumor_respawn_time: i64,
    wagon_respawn_time: i64,
    conquest_ended: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            worlds: Worlds::new(),
            preawakened_echmus: None,
            coffins_created: 0,
            tumor_respawn_time: 180 * 1000,
            wagon_respawn_time: 60 * 1000,
            conquest_ended: false,
        }
    }
}

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut State {
    ctx.state::<State>()
}

fn hid<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32) -> &'a mut HidWorld {
    state(ctx).worlds.entry(instance_id).or_default()
}

fn world_of(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let instance_id = common::world_of(ctx, object)?;
    state(ctx).worlds.contains_key(&instance_id).then_some(instance_id)
}

/// Java `ai.areas.Gracia.instances.HeartInfinityDefence.HeartInfinityDefence`.
pub struct HeartInfinityDefence;

impl HeartInfinityDefence {
    fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
        if ctx.is_gm(player) {
            return true;
        }

        if ctx.world().party_of(player).is_none() {
            ctx.send_system_message(player, SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter);
            return false;
        }

        if !common::is_party_leader(ctx, player) {
            ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            return false;
        }

        if ctx.world().command_channel_of(player).is_none() || !common::is_channel_leader(ctx, player) {
            ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            return false;
        }

        let seeds = ctx.script_settings().gracia_seeds.clone();
        common::check_channel_members(
            ctx,
            player,
            INSTANCEID,
            seeds.heart_defence_min_players,
            seeds.heart_defence_max_players,
            None,
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
            hid(ctx, instance_id);
            tracing::info!(
                "Heart Infinity Defence started {INSTANCEID} Instance: {instance_id} created by player: {}",
                ctx.player_name(player)
            );

            if ctx.is_gm(player) {
                common::teleport_into(ctx, player, ENTER_TELEPORT, instance_id);
                common::add_allowed(ctx, instance_id, player);
            }

            if ctx.world().command_channel_of(player).is_none() {
                common::teleport_into(ctx, player, ENTER_TELEPORT, instance_id);
                common::add_allowed(ctx, instance_id, player);
            } else {
                for party_member in common::channel_members(ctx, player) {
                    common::teleport_into(ctx, party_member, ENTER_TELEPORT, instance_id);
                    common::add_allowed(ctx, instance_id, party_member);
                    if ctx.player_quest_state(party_member, QN).is_none() {
                        common::new_quest_state(ctx, party_member);
                    }
                }
            }

            let now = ctx.now_ms();
            let world = hid(ctx, instance_id);
            world.start_time = now;
            world.finish_task = true;
            world.timer_task = true;
            ctx.start_quest_timer(&format!("FinishTask {instance_id}"), 30 * 60000, None, None, false);
            // `scheduleAtFixedRate(new TimerTask(world), 298 * 1000, 5 * 60 * 1000)`.
            ctx.start_quest_timer(&format!("TimerTask {instance_id}"), 298 * 1000, None, None, false);
            // `conquestBegins(world)`: everything starts 20 seconds later.
            ctx.start_quest_timer(&format!("ConquestBegins {instance_id}"), 20000, None, None, false);
        }
    }

    /// The task of `conquestBegins(world)`.
    fn conquest_begins(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::YouCanHearTheUndeadOfEkimusRushingTowardYouS1S2ItHasNowBegun,
            &[],
        );
        for spawn1 in &ROOMS_MOBS {
            common::spawn_with_respawn(ctx, spawn1, instance_id);
        }

        for spawn2 in &ROOMS_TUMORS {
            for _ in 0..spawn2[6] {
                if let Some(npc2) = common::spawn(ctx, spawn2[0], (spawn2[1], spawn2[2], spawn2[3], spawn2[4]), instance_id) {
                    hid(ctx, instance_id).dead_tumors.push(npc2);
                }
            }
        }

        ctx.open_door(14240102, instance_id);
        let echmus = ctx.add_spawn(29161, -179534, 208510, -15496, 16342, false, 0, instance_id);
        state(ctx).preawakened_echmus = echmus;

        ctx.start_quest_timer(&format!("SpawnCoffins {instance_id}"), 60000, None, None, false);
        let respawn = state(ctx).tumor_respawn_time;
        ctx.start_quest_timer(&format!("AliveTumors {instance_id}"), respawn, None, None, false);

        // `wagonSpawnTask = scheduleAtFixedRate(.., 1000, wagonRespawnTime)`: the period is the
        // value of now; later changes of `wagonRespawnTime` do not reach the running task.
        let period = state(ctx).wagon_respawn_time;
        hid(ctx, instance_id).wagon_spawn_task = true;
        ctx.start_quest_timer(&format!("WagonSpawn {instance_id} {period}"), 1000, None, None, false);
    }

    /// Java `notifyWagonArrived(npc, world)`.
    fn notify_wagon_arrived(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        state(ctx).coffins_created += 1;
        let coffins_created = state(ctx).coffins_created;
        if coffins_created == 20 {
            Self::conquest_conclusion(ctx, instance_id);
        } else {
            if let Some(echmus) = state(ctx).preawakened_echmus {
                ctx.broadcast_npc_say(echmus, ChatType::Shout, NpcStringId::BringMoreMoreSouls, &[]);
            }
            let left = (MAX_COFFINS - coffins_created).to_string();
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::TheSoulCoffinHasAwakenedEkimusIfS1MoreSoulCoffinSAreCreatedTheDefenseOfTheHeartOfImmortalityWillFail,
                &[&left],
            );
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

    /// Java `conquestConclusion(world)`: the defence failed.
    fn conquest_conclusion(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let world = hid(ctx, instance_id);
        let (timer, finish, wagon) = (world.timer_task, world.finish_task, world.wagon_spawn_task);
        world.timer_task = false;
        world.finish_task = false;
        world.wagon_spawn_task = false;
        if timer {
            ctx.cancel_quest_timer(&format!("TimerTask {instance_id}"), None, None);
        }
        if finish {
            ctx.cancel_quest_timer(&format!("FinishTask {instance_id}"), None, None);
        }
        let _ = wagon; // the next `WagonSpawn` sees the flag and stops

        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::YouHaveFailedAtS1S2TheInstanceWillShortlyExpire,
            &[],
        );

        state(ctx).conquest_ended = true;
        common::conclude(ctx, instance_id);
    }

    /// The timers that are Java's `ThreadPool` tasks.
    fn on_task(ctx: &mut ScriptCtx<'_>, event: &str) -> bool {
        let mut parts = event.split(' ');
        let task = parts.next().unwrap_or_default();
        let Some(instance_id) = parts.next().and_then(|v| v.parse::<i32>().ok()) else {
            return false;
        };
        let arg = parts.next().and_then(|v| v.parse::<i64>().ok());
        match task {
            "ConquestBegins" => Self::conquest_begins(ctx, instance_id),
            "FinishTask" => Self::finish_task(ctx, instance_id),
            "TimerTask" => {
                let start = hid(ctx, instance_id).start_time;
                let time = ((start + (25 * 60 * 1000)) - ctx.now_ms()) / 60000;
                if time == 0 {
                    Self::conquest_conclusion(ctx, instance_id);
                } else if time == 15 {
                    for spawn in &ROOMS_BOSSES {
                        for _ in 0..spawn[6] {
                            common::spawn(ctx, spawn[0], (spawn[1], spawn[2], spawn[3], spawn[4]), instance_id);
                        }
                    }
                }
                if hid(ctx, instance_id).timer_task && ctx.world().instance(instance_id).is_some() {
                    ctx.start_quest_timer(&format!("TimerTask {instance_id}"), 5 * 60 * 1000, None, None, false);
                }
            }
            "SpawnCoffins" => {
                if !state(ctx).conquest_ended {
                    for npc in hid(ctx, instance_id).dead_tumors.clone() {
                        // `spawnCoffin(npc, world)`.
                        let loc = npc.location(ctx);
                        common::spawn(ctx, REGENERATION_COFFIN, loc, instance_id);
                    }
                }
            }
            "AliveTumors" => {
                if !state(ctx).conquest_ended {
                    let dead = std::mem::take(&mut hid(ctx, instance_id).dead_tumors);
                    for npc3 in dead {
                        ctx.delete_npc(npc3.npc);
                    }

                    for spawn in &ROOMS_ALIVE_TUMORS {
                        for _ in 0..spawn[6] {
                            if let Some(npc4) =
                                common::spawn(ctx, spawn[0], (spawn[1], spawn[2], spawn[3], spawn[4]), instance_id)
                            {
                                common::set_hp_ratio(ctx, npc4.npc.object_id, 0.5);
                                hid(ctx, instance_id).dead_tumors.push(npc4);
                            }
                        }
                    }

                    common::screen_message(
                        ctx,
                        instance_id,
                        NpcStringId::TheTumorInsideS1HasCompletelyRevivedNekimusStartedToRegainHisEnergyAndIsDesperatelyLookingForHisPrey,
                        &[],
                    );
                }
            }
            "WagonSpawn" => {
                if !hid(ctx, instance_id).wagon_spawn_task || ctx.world().instance(instance_id).is_none() {
                    return true;
                }
                ctx.add_spawn(SOULWAGON, -179544, 207400, -15496, 0, false, 0, instance_id);
                let period = arg.unwrap_or(60000);
                ctx.start_quest_timer(&format!("WagonSpawn {instance_id} {period}"), period, None, None, false);
            }
            "TumorRevival" => {
                // The arguments: the place of the tumor that died.
                let loc: Vec<i32> = event
                    .split(' ')
                    .skip(2)
                    .filter_map(|v| v.parse().ok())
                    .collect();
                let [x, y, z, heading] = loc[..] else {
                    return true;
                };
                if let Some(dead_tumor) = hid(ctx, instance_id).dead_tumor {
                    ctx.delete_npc(dead_tumor.npc);
                }
                if let Some(alivetumor) = common::spawn(ctx, ALIVETUMOR, (x, y, z, heading), instance_id) {
                    common::set_hp_ratio(ctx, alivetumor.npc.object_id, 0.25);
                    hid(ctx, instance_id).npc_list.push(alivetumor);
                }
                state(ctx).wagon_respawn_time -= 10000;
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::TheTumorInsideS1HasCompletelyRevivedNekimusStartedToRegainHisEnergyAndIsDesperatelyLookingForHisPrey,
                    &[],
                );
            }
            _ => return false,
        }
        true
    }
}

impl Script for HeartInfinityDefence {
    fn name(&self) -> &'static str {
        QN
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/instances/HeartInfinityDefence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ABYSSGAZE]);
        s.add_talk_id(&[ABYSSGAZE]);
        s.add_start_npc(&[DEADTUMOR]);
        s.add_talk_id(&[DEADTUMOR]);
        s.add_start_npc(&[32536]);
        s.add_talk_id(&[32536]);

        for id in NOTMOVE {
            s.add_spawn_id(&[id]);
        }

        s.add_spawn_id(&[SOULWAGON]);

        s.add_aggro_range_enter_id(&[18668]);

        s.add_kill_id(&[ALIVETUMOR]);
        s.add_kill_id(&[18711]);

        s.add_enter_zone_id(&[200032]);
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

        if event.starts_with("warpechmus") {
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::S1SPartyHasMovedToADifferentLocationThroughTheCrackInTheTumor,
                &[],
            );
            common::teleport_party_near(ctx, player, 800.0, (-179548, 209584, -15504));
        } else if event.starts_with("reenterechmus") {
            ctx.world_mut().destroy_item_by_item_id(player, 13797, 3, true);
            common::teleport_party_near(ctx, player, 400.0, (-179548, 209584, -15504));
        } else if event.starts_with("warp") {
            let (mut dead_tumors, dead_tumor) = {
                let world = hid(ctx, instance_id);
                (std::mem::take(&mut world.dead_tumors), world.dead_tumor)
            };
            common::warp(ctx, instance_id, player, &mut dead_tumors, dead_tumor);
            hid(ctx, instance_id).dead_tumors = dead_tumors;
        }

        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.player_quest_state(player, QN).is_none() {
            common::new_quest_state(ctx, player);
        }

        if npc.id == ABYSSGAZE {
            Self::enter_instance(ctx, player);
        }

        None
    }

    fn on_aggro_range_enter(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId, _is_summon: bool) {
        if let Some(instance_id) = world_of(ctx, npc.object_id)
            && npc.id == 18668
        {
            common::ward_of_death(ctx, npc, instance_id, &MONSTERS, true);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if NOTMOVE.contains(&npc.id) {
            ctx.set_random_walking(npc, false);
            ctx.set_immobilized(npc, true);
        }

        if npc.id == SOULWAGON && world_of(ctx, npc.object_id).is_some() {
            // npc.asMonster().setPassive(true);
            ctx.world_mut().script_set_intention_idle(npc.object_id);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        let loc = common::location_of(ctx, npc.object_id);
        if npc.id == ALIVETUMOR {
            let count = i64::from(ctx.get_random_range(2, 5));
            ctx.drop_item(npc, Some(player), 13797, count);
            ctx.delete_npc(npc);
            let dead_tumor = loc.and_then(|loc| common::spawn(ctx, DEADTUMOR, loc, instance_id));
            {
                let world = hid(ctx, instance_id);
                world.dead_tumor = dead_tumor;
                if let Some(dead) = dead_tumor {
                    world.dead_tumors.push(dead);
                }
            }
            state(ctx).wagon_respawn_time += 10000;
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::TheTumorInsideS1HasBeenDestroyedNtheSpeedThatEkimusCallsOutHisPreyHasSlowedDown,
                &[],
            );

            if let Some((x, y, z, heading)) = loc {
                let respawn = state(ctx).tumor_respawn_time;
                let name = format!("TumorRevival {instance_id} {x} {y} {z} {heading}");
                ctx.start_quest_timer(&name, respawn, None, None, false);
            }
        }

        if npc.id == 18711 {
            state(ctx).tumor_respawn_time += 5 * 1000;
        }
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        let Some(npc) = ctx.script_npc(creature) else {
            return;
        };
        if !ctx.world().is_attackable_npc(creature) {
            return;
        }
        if let Some(instance_id) = world_of(ctx, creature)
            && npc.id == SOULWAGON
        {
            Self::notify_wagon_arrived(ctx, instance_id);
            ctx.delete_npc(npc);
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
        <npc id="32539" type="Folk" level="70" name="Abyss Gaze"/>
        <npc id="32535" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="18708" type="Monster" level="81" name="Tumor of Death">
            <stats><vitals hp="1000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="18709" type="Monster" level="80" name="Soul Coffin"/>
        <npc id="29161" type="Monster" level="82" name="Ekimus"/>
        <npc id="22523" type="Monster" level="80" name="Soul Wagon"/>"#;
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
    fn the_heart_holds_thirty_minutes_against_the_soul_wagons() {
        let mut registry = ScriptRegistry::new();
        registry.add(HeartInfinityDefence);
        registry.add(Quest697);
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &[], |world, _| {
            world.set_script_settings(l2_world::ScriptSettings {
                gracia_seeds: l2_world::script::GraciaSeedsSettings {
                    heart_defence_min_players: 2,
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
        let gaze = ScriptNpc {
            object_id: t.spawn(ABYSSGAZE, 20),
            id: ABYSSGAZE,
        };
        t.world.run_as_script(QN, |ctx| {
            HeartInfinityDefence.on_talk(ctx, gaze, Oid(1));
        });
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_world(instance).unwrap().allowed.len(), 4);

        // Twenty seconds: the dead tumors, Ekimus asleep, the first Soul Wagon a second later.
        t.wait_ms(21_100);
        assert_eq!(t.npcs_of(DEADTUMOR).len(), 6);
        assert_eq!(t.npcs_of(29161).len(), 1);
        assert_eq!(t.npcs_of(SOULWAGON).len(), 1);
        // A wagon every minute; a coffin at every dead tumor after a minute.
        t.wait_ms(60_000);
        assert_eq!(t.npcs_of(SOULWAGON).len(), 2);
        assert_eq!(t.npcs_of(REGENERATION_COFFIN).len(), 6);
        // Three minutes after the start of the fight the tumors are alive, at half health.
        t.wait_ms(120_000);
        let tumors = alive(&t, ALIVETUMOR);
        assert_eq!(tumors.len(), 6);
        assert!(t.npcs_of(DEADTUMOR).is_empty());
        // A tumor killed leaves a dead one that comes alive again three minutes later.
        t.kill(1, tumors[0]);
        assert_eq!(t.npcs_of(DEADTUMOR).len(), 1);
        t.wait_ms(180_000);
        assert!(t.npcs_of(DEADTUMOR).is_empty());
        assert_eq!(alive(&t, ALIVETUMOR).len(), 6);

        // Thirty minutes after the entry the defence held.
        t.take_writes();
        let finish = format!("FinishTask {instance}");
        t.world.run_as_script(QN, |ctx| ctx.notify_event(&finish, None, None));
        let writes = t.take_writes();
        assert!(
            writes.iter().any(|w| matches!(w,
                QuestWrite::InsertVar { char_id: 2, var, value, .. } if var == "defenceDone" && value == "1")),
            "{writes:?}"
        );
        assert!(t.npcs_of(29161).is_empty());
    }

    #[test]
    fn the_timer_task_ends_the_defence_at_25_minutes() {
        let mut registry = ScriptRegistry::new();
        registry.add(HeartInfinityDefence);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.world.run_as_script(QN, |ctx| {
            let instance = common::create_world(ctx, INSTANCEID);
            let now = ctx.now_ms();
            let world = hid(ctx, instance);
            // NOTE(java-quirk): the task runs at 298 s + 5 min * n, so at 24 min 58 s
            // `(start + 25 min - now) / 60000` is 0: the defence fails before `FinishTask`.
            world.start_time = now - (25 * 60_000 - 2_000);
            world.timer_task = true;
            world.finish_task = true;
            ctx.notify_event(&format!("TimerTask {instance}"), None, None);
            assert!(state(ctx).conquest_ended);
            assert!(!hid(ctx, instance).finish_task);
            assert!(!ctx.has_quest_timer(&format!("TimerTask {instance}"), None, None));
        });
    }
}
