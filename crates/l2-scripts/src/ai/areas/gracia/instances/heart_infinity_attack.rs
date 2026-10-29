//! The Heart of Infinity (attack): a command channel cuts Ekimus off his six Tumors of Death and
//! kills him for the Seed of Infinity.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HeartInfinityAttack/HeartInfinityAttack.java
//
// NOTE(java-quirk): `handleEkimusStats` is a `TODO FIXME` in Java (its body is commented out);
// Ekimus' stats do not follow the tumors.

use l2_protocol_game::server::ChatType;
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::common::{self, NpcAt, Worlds};
use crate::ai::areas::gracia::seeds::{SOI_MANAGER, SOI_NOTIFY_EKIMUS_KILL};
use crate::ai::areas::gracia::util::players_in_zone;

const QN: &str = "HeartInfinityAttack";

const INSTANCEID: i32 = 121;
const INSTANCEPENALTY: i64 = 24;

/// `ZoneManager.getInstance().getZoneByName("Heart of Infinity Attack")`.
const ZONE: &str = "Heart of Infinity Attack";

const ABYSSGAZE: i32 = 32539;
const ALIVETUMOR: i32 = 18708;
const DEADTUMOR: i32 = 32535;
const EKIMUS: i32 = 29150;
const HOUND: i32 = 29151;
const COFFIN: i32 = 18710;

/// Java `SC_ECHMUS_OPENING`, `SC_ECHMUS_SUCCESS`.
const SC_ECHMUS_OPENING: i32 = 2;
const SC_ECHMUS_SUCCESS: i32 = 3;

const ENTER_TELEPORT: (i32, i32, i32) = (-179284, 205990, -15520);

const NOTMOVE: [i32; 4] = [18668, 18711, ALIVETUMOR, DEADTUMOR];

const MONSTERS: [i32; 4] = [22516, 22520, 22522, 22524];

// @formatter:off
/// `ROOMS_MOBS` (the Defence has the same table).
pub(crate) const ROOMS_MOBS: [[i32; 7]; 72] = [
    [22520, -179949, 206751, -15521, 0, 60, 1],
    [22522, -179949, 206505, -15522, 0, 60, 1],
    [22532, -180207, 206362, -15512, 0, 60, 1],
    [22520, -180046, 206109, -15511, 0, 60, 1],
    [22526, -179843, 205686, -15511, 0, 60, 1],
    [22522, -179658, 206002, -15518, 0, 60, 1],
    [22526, -179465, 205648, -15498, 0, 60, 1],
    [22520, -179262, 205896, -15536, 0, 60, 1],
    [22526, -178907, 205950, -15509, 0, 60, 1],
    [22532, -179128, 206423, -15528, 0, 60, 1],
    [22522, -179262, 206890, -15524, 0, 60, 1],
    [22516, -177238, 208020, -15521, 0, 60, 1],
    [22522, -177202, 207533, -15516, 0, 60, 1],
    [22532, -176900, 207515, -15511, 0, 60, 1],
    [22524, -176847, 208022, -15523, 0, 60, 1],
    [22516, -176187, 207648, -15499, 0, 60, 1],
    [22522, -176044, 208039, -15524, 0, 60, 1],
    [22532, -176346, 208233, -15533, 0, 60, 1],
    [22524, -176440, 208594, -15527, 0, 60, 1],
    [22516, -176938, 208543, -15529, 0, 60, 1],
    [22524, -176763, 208108, -15523, 0, 60, 1],
    [22522, -176483, 207762, -15520, 0, 60, 1],
    [22526, -177125, 210766, -15517, 0, 60, 1],
    [22520, -176810, 210587, -15515, 0, 60, 1],
    [22522, -176820, 210975, -15528, 0, 60, 1],
    [22524, -176412, 210873, -15510, 0, 60, 1],
    [22526, -176261, 211197, -15511, 0, 60, 1],
    [22520, -176241, 211649, -15503, 0, 60, 1],
    [22522, -176656, 211884, -15516, 0, 60, 1],
    [22524, -176798, 211540, -15541, 0, 60, 1],
    [22526, -177130, 211694, -15533, 0, 60, 1],
    [22520, -177436, 211384, -15519, 0, 60, 1],
    [22522, -177138, 211159, -15524, 0, 60, 1],
    [22526, -179311, 212338, -15521, 0, 60, 1],
    [22522, -178985, 212502, -15513, 0, 60, 1],
    [22516, -179313, 212730, -15522, 0, 60, 1],
    [22520, -178966, 213036, -15510, 0, 60, 1],
    [22526, -179352, 213172, -15511, 0, 60, 1],
    [22522, -179696, 213427, -15514, 0, 60, 1],
    [22516, -180096, 213140, -15519, 0, 60, 1],
    [22520, -179957, 212724, -15530, 0, 60, 1],
    [22522, -180240, 212578, -15518, 0, 60, 1],
    [22520, -179891, 212271, -15520, 0, 60, 1],
    [22516, -179593, 212488, -15523, 0, 60, 1],
    [22516, -181746, 211163, -15515, 0, 60, 1],
    [22522, -182002, 211394, -15522, 0, 60, 1],
    [22526, -181796, 211647, -15511, 0, 60, 1],
    [22516, -182160, 211615, -15518, 0, 60, 1],
    [22520, -182322, 211843, -15510, 0, 60, 1],
    [22522, -182720, 211686, -15519, 0, 60, 1],
    [22520, -182721, 211231, -15535, 0, 60, 1],
    [22516, -182957, 210874, -15515, 0, 60, 1],
    [22526, -182705, 210606, -15526, 0, 60, 1],
    [22520, -182359, 210871, -15526, 0, 60, 1],
    [22522, -182051, 210644, -15521, 0, 60, 1],
    [22524, -181994, 208452, -15514, 0, 60, 1],
    [22516, -182342, 208661, -15510, 0, 60, 1],
    [22526, -182471, 208349, -15519, 0, 60, 1],
    [22520, -182960, 208335, -15504, 0, 60, 1],
    [22524, -182751, 207925, -15514, 0, 60, 1],
    [22516, -182671, 207442, -15531, 0, 60, 1],
    [22520, -182276, 207438, -15527, 0, 60, 1],
    [22526, -181975, 207353, -15520, 0, 60, 1],
    [22524, -181989, 207795, -15527, 0, 60, 1],
    [22516, -181744, 207997, -15520, 0, 60, 1],
    [22520, -182318, 208070, -15523, 0, 60, 1],
    [18667, -178430, 211520, -15504, 28672, 120, 1],
    [18668, -177341, 209129, -15504, 16384, 120, 1],
    [18711, -178067, 207862, -15504, 8192, 120, 1],
    [18667, -180679, 207614, -15520, 61439, 120, 1],
    [18668, -181761, 209949, -15520, 49151, 120, 1],
    [18711, -180328, 211686, -15504, 36863, 120, 1],
];

pub(crate) const ROOMS_TUMORS: [[i32; 7]; 6] = [
    [18708, -179779, 212540, -15520, 49151, 0, 1],
    [18708, -177028, 211135, -15520, 36863, 0, 1],
    [18708, -176355, 208043, -15520, 28672, 0, 1],
    [18708, -179284, 205990, -15520, 16384, 0, 1],
    [18708, -182268, 208218, -15520, 4096, 0, 1],
    [18708, -182069, 211140, -15520, 61439, 0, 1],
];

/// `ROOMS_BOSSES` (the Defence has the same table).
pub(crate) const ROOMS_BOSSES: [[i32; 7]; 6] = [
    [25637, -179303, 213090, -15504, 49151, 0, 1],
    [25638, -176426, 211219, -15504, 36863, 0, 1],
    [25639, -177040, 207870, -15504, 28672, 0, 1],
    [25640, -179762, 206479, -15504, 16384, 0, 1],
    [25641, -182388, 207599, -15504, 4096, 0, 1],
    [25642, -182733, 211096, -15504, 61439, 0, 1],
];
// @formatter:on

/// Java `HIAWorld`.
#[derive(Debug, Default)]
struct HiaWorld {
    npc_list: Vec<NpcAt>,
    startroom: Vec<NpcAt>,
    dead_tumors: Vec<NpcAt>,
    ekimus: Option<ScriptNpc>,
    dead_tumor: Option<NpcAt>,
    hounds: Vec<ScriptNpc>,
    tumor_count: i32,
    is_boss_attacked: bool,
    start_time: i64,
    /// `timerTask != null` and not cancelled.
    timer_task: bool,
}

/// The script's fields and its worlds.
// NOTE(java-quirk): `tumorRespawnTime`, `conquestBegun`, `conquestEnded` and `houndBlocked` are
// fields of the script, shared by every instance: once one channel reached Ekimus, the opening
// of no other instance runs again until the server restarts.
struct State {
    worlds: Worlds<HiaWorld>,
    tumor_respawn_time: i64,
    conquest_begun: bool,
    conquest_ended: bool,
    hound_blocked: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            worlds: Worlds::new(),
            tumor_respawn_time: 180 * 1000,
            conquest_begun: false,
            conquest_ended: false,
            hound_blocked: false,
        }
    }
}

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut State {
    ctx.state::<State>()
}

fn hia<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32) -> &'a mut HiaWorld {
    state(ctx).worlds.entry(instance_id).or_default()
}

fn world_of(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let instance_id = common::world_of(ctx, object)?;
    state(ctx)
        .worlds
        .contains_key(&instance_id)
        .then_some(instance_id)
}

/// Java `playMovie(world, movie)`: the allowed players inside.
pub(crate) fn play_movie(ctx: &mut ScriptCtx<'_>, instance_id: i32, movie: i32) {
    for player in common::allowed(ctx, instance_id) {
        if ctx.world().instance_id_of(player) == instance_id {
            ctx.play_movie(player, movie);
        }
    }
}

/// Java `ai.areas.Gracia.instances.HeartInfinityAttack.HeartInfinityAttack`.
pub struct HeartInfinityAttack;

impl HeartInfinityAttack {
    fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
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

        if ctx.world().command_channel_of(player).is_none() || !common::is_channel_leader(ctx, player) {
            ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            return false;
        }

        let seeds = ctx.script_settings().gracia_seeds.clone();
        common::check_channel_members(
            ctx,
            player,
            INSTANCEID,
            seeds.heart_attack_min_players,
            seeds.heart_attack_max_players,
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
            hia(ctx, instance_id);
            tracing::info!(
                "Heart Infinity Attack started {INSTANCEID} Instance: {instance_id} created by player: {}",
                ctx.player_name(player)
            );

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

            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::YouWillParticipateInS1S2ShortlyBePreparedForAnything,
                &[],
            );
            if let Some(npc) = common::spawn(ctx, 32536, (-179376, 206111, -15538, 16384), instance_id) {
                hia(ctx, instance_id).startroom.push(npc);
            }
        }
    }

    /// Java `notifyEchmusEntrance(world)`.
    fn notify_echmus_entrance(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        if state(ctx).conquest_begun {
            return;
        }

        state(ctx).conquest_begun = true;
        ctx.start_quest_timer(&format!("EchmusOpening {instance_id}"), 20000, None, None, false);
    }

    /// Java `conquestBegins(world)`.
    fn conquest_begins(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let startroom = std::mem::take(&mut hia(ctx, instance_id).startroom);
        for npc in startroom {
            ctx.delete_npc(npc.npc);
        }

        for spawn in &ROOMS_TUMORS {
            for _ in 0..spawn[6] {
                if let Some(npc) = common::spawn(
                    ctx,
                    spawn[0],
                    (spawn[1], spawn[2], spawn[3], spawn[4]),
                    instance_id,
                ) {
                    common::set_hp_ratio(ctx, npc.npc.object_id, 0.5);
                    hia(ctx, instance_id).npc_list.push(npc);
                }
            }
        }

        for spawn in &ROOMS_MOBS {
            common::spawn_with_respawn(ctx, spawn, instance_id);
        }

        ctx.open_door(14240102, instance_id);
        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::YouCanHearTheUndeadOfEkimusRushingTowardYouS1S2ItHasNowBegun,
            &[],
        );
        let ekimus = ctx.add_spawn(EKIMUS, -179537, 208854, -15504, 16384, false, 0, instance_id);
        if let Some(ekimus) = ekimus {
            ctx.broadcast_npc_say(
                ekimus,
                ChatType::Shout,
                NpcStringId::IShallAcceptYourChallengeS1ComeAndDieInTheArmsOfImmortality,
                &[],
            );
        }
        let hounds: Vec<ScriptNpc> = [(-179224, 209624), (-179880, 209464)]
            .into_iter()
            .filter_map(|(x, y)| ctx.add_spawn(HOUND, x, y, -15504, 16384, false, 0, instance_id))
            .collect();
        let now = ctx.now_ms();
        let world = hia(ctx, instance_id);
        world.ekimus = ekimus;
        world.hounds.extend(hounds);
        world.start_time = now;
        world.timer_task = true;
        // `scheduleAtFixedRate(new TimerTask(world), 298 * 1000, 5 * 60 * 1000)`.
        ctx.start_quest_timer(&format!("TimerTask {instance_id}"), 298 * 1000, None, None, false);
    }

    /// Java `TimerTask.run()`.
    fn timer_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        let start = hia(ctx, instance_id).start_time;
        let time = ((start + (25 * 60 * 1000)) - ctx.now_ms()) / 60000;
        if time == 0 {
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::YouHaveFailedAtS1S2TheInstanceWillShortlyExpire,
                &[],
            );
            state(ctx).conquest_ended = true;
            common::conclude(ctx, instance_id);
        } else {
            for spawn in &ROOMS_BOSSES {
                for _ in 0..spawn[6] {
                    common::spawn(
                        ctx,
                        spawn[0],
                        (spawn[1], spawn[2], spawn[3], spawn[4]),
                        instance_id,
                    );
                }
            }
        }
        // NOTE(java-quirk): the task runs on after the failure until the instance is gone.
        if hia(ctx, instance_id).timer_task && ctx.world().instance(instance_id).is_some() {
            ctx.start_quest_timer(
                &format!("TimerTask {instance_id}"),
                5 * 60 * 1000,
                None,
                None,
                false,
            );
        }
    }

    /// Java `notifyTumorRevival(world)`.
    fn notify_tumor_revival(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        if hia(ctx, instance_id).tumor_count == 1 && state(ctx).hound_blocked {
            state(ctx).hound_blocked = false;
            for hound in hia(ctx, instance_id).hounds.clone() {
                ctx.world_mut().set_invul(hound.object_id, false);
            }

            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::WithTheConnectionToTheTumorRestoredEkimusHasRegainedControlOverTheFeralHound,
                &[],
            );
        } else {
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::TheTumorInsideS1HasBeenCompletelyResurrectedNAndStartedToEnergizeEkimusAgain,
                &[],
            );
        }
    }

    /// Java `notifyEkimusRoomEntrance(world)`.
    // NOTE(java-quirk): the players of the zone in every instance are expelled.
    fn notify_ekimus_room_entrance(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        if let Some(zone) = ctx.world().zone_id_by_name(ZONE) {
            for player in players_in_zone(ctx, zone) {
                let offset = ctx.world().max_offset_on_teleport();
                ctx.world_mut().tele_to_location(
                    player,
                    l2_world::teleport::TeleportTarget::at(-179537, 211233, -15472)
                        .with_random_offset(offset),
                );
            }
        }

        ctx.start_quest_timer(&format!("EkimusExpelled {instance_id}"), 10000, None, None, false);
    }

    /// Java `conquestConclusion(world)`.
    fn conquest_conclusion(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
        if hia(ctx, instance_id).timer_task {
            hia(ctx, instance_id).timer_task = false;
            ctx.cancel_quest_timer(&format!("TimerTask {instance_id}"), None, None);
        }

        common::screen_message(
            ctx,
            instance_id,
            NpcStringId::CongratulationsYouHaveSucceededAtS1S2TheInstanceWillShortlyExpire,
            &[],
        );
        state(ctx).conquest_ended = true;
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
            "EchmusOpening" => {
                play_movie(ctx, instance_id, SC_ECHMUS_OPENING);
                ctx.start_quest_timer(&format!("ConquestBegins {instance_id}"), 62500, None, None, false);
            }
            "ConquestBegins" => Self::conquest_begins(ctx, instance_id),
            "TimerTask" => Self::timer_task(ctx, instance_id),
            "EkimusExpelled" => common::screen_message(
                ctx,
                instance_id,
                NpcStringId::EkimusHasSensedAbnormalActivityNtheAdvancingPartyIsForcefullyExpelled,
                &[],
            ),
            "TumorRevival" | "RegenerationCoffinSpawn" => {
                if state(ctx).conquest_ended {
                    return true;
                }
                let Some(dead_tumor) = arg.and_then(|id| {
                    state(ctx)
                        .worlds
                        .get(&instance_id)
                        .and_then(|w| w.dead_tumors.iter().find(|t| t.npc.object_id.0 == id).copied())
                }) else {
                    return true;
                };
                let loc = dead_tumor.location(ctx);
                if task == "TumorRevival" {
                    let alivetumor = common::spawn(ctx, ALIVETUMOR, loc, instance_id);
                    if let Some(alivetumor) = alivetumor {
                        common::set_hp_ratio(ctx, alivetumor.npc.object_id, 0.25);
                    }
                    Self::notify_tumor_revival(ctx, instance_id);
                    if let Some(alivetumor) = alivetumor {
                        hia(ctx, instance_id).npc_list.push(alivetumor);
                    }
                    ctx.delete_npc(dead_tumor.npc);
                    common::add_tag(ctx, instance_id, -1);
                } else {
                    for _ in 0..4 {
                        if let Some(worm) = common::spawn(ctx, COFFIN, loc, instance_id) {
                            hia(ctx, instance_id).npc_list.push(worm);
                        }
                    }
                }
            }
            _ => return false,
        }
        true
    }
}

impl Script for HeartInfinityAttack {
    fn name(&self) -> &'static str {
        QN
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/instances/HeartInfinityAttack"
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

        s.add_aggro_range_enter_id(&[18668]);

        s.add_attack_id(&[EKIMUS]);

        s.add_kill_id(&[ALIVETUMOR]);
        s.add_kill_id(&[EKIMUS]);
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

        if event.starts_with("warpechmus") {
            common::screen_message(
                ctx,
                instance_id,
                NpcStringId::S1SPartyHasMovedToADifferentLocationThroughTheCrackInTheTumor,
                &[],
            );
            common::teleport_party_near(ctx, player, 800.0, (-179548, 209584, -15504));

            Self::notify_echmus_entrance(ctx, instance_id);
        } else if event.starts_with("reenterechmus") {
            ctx.world_mut().destroy_item_by_item_id(player, 13797, 3, true);
            Self::notify_ekimus_room_entrance(ctx, instance_id);
            common::teleport_party_near(ctx, player, 400.0, (-179548, 209584, -15504));
        } else if event.starts_with("warp") {
            let (mut dead_tumors, dead_tumor) = {
                let world = hia(ctx, instance_id);
                (std::mem::take(&mut world.dead_tumors), world.dead_tumor)
            };
            common::warp(ctx, instance_id, player, &mut dead_tumors, dead_tumor);
            hia(ctx, instance_id).dead_tumors = dead_tumors;
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
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        if !hia(ctx, instance_id).is_boss_attacked {
            hia(ctx, instance_id).is_boss_attacked = true;
            common::restrict_entry(ctx, instance_id, INSTANCEID, INSTANCEPENALTY, false);
        }

        if npc.id == EKIMUS {
            for mob in hia(ctx, instance_id).hounds.clone() {
                ctx.world_mut().add_damage_hate(mob.object_id, attacker, 0, 500);
                ctx.set_running(mob);
                ctx.set_intention_attack(mob, attacker);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if NOTMOVE.contains(&npc.id) || npc.id == HOUND {
            ctx.set_random_walking(npc, false);
            ctx.set_immobilized(npc, true);
        }

        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        if npc.id == ALIVETUMOR {
            hia(ctx, instance_id).tumor_count += 1;
        }

        if npc.id == DEADTUMOR {
            common::add_tag(ctx, instance_id, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(instance_id) = world_of(ctx, npc.object_id) else {
            return;
        };
        let loc = common::location_of(ctx, npc.object_id);
        if npc.id == ALIVETUMOR {
            hia(ctx, instance_id).tumor_count -= 1;
            let count = i64::from(ctx.get_random_range(2, 5));
            ctx.drop_item(npc, Some(player), 13797, count);
            ctx.delete_npc(npc);
            let dead_tumor = loc.and_then(|loc| common::spawn(ctx, DEADTUMOR, loc, instance_id));
            {
                let world = hia(ctx, instance_id);
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
            if hia(ctx, instance_id).tumor_count < 1 {
                state(ctx).hound_blocked = true;
                for hound in hia(ctx, instance_id).hounds.clone() {
                    ctx.world_mut().set_invul(hound.object_id, true);
                }

                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::WithAllConnectionsToTheTumorSeveredEkimusHasLostItsPowerToControlTheFeralHound,
                    &[],
                );
            } else {
                common::screen_message(
                    ctx,
                    instance_id,
                    NpcStringId::TheTumorInsideS1ThatHasProvidedEnergyNToEkimusIsDestroyed,
                    &[],
                );
            }
        }

        if npc.id == EKIMUS {
            Self::conquest_conclusion(ctx, instance_id);
            ctx.notify_event_of(SOI_MANAGER, SOI_NOTIFY_EKIMUS_KILL, None, None);
            play_movie(ctx, instance_id, SC_ECHMUS_SUCCESS);
        }

        if npc.id == 18711 {
            state(ctx).tumor_respawn_time += 8 * 1000;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::gracia::seeds::SeedOfInfinityManager;
    use crate::ai::areas::gracia::testing::{STRATEGY_GUIDE, command_channel};
    use crate::testing::{ScriptTest, item, player_data};

    pub(crate) const NPCS: &str = r#"
        <npc id="32539" type="Folk" level="70" name="Abyss Gaze"/>
        <npc id="32535" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="32536" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="18708" type="Monster" level="81" name="Tumor of Death">
            <stats><vitals hp="1000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="18710" type="Monster" level="81" name="Soul Coffin"/>
        <npc id="29150" type="GrandBoss" level="82" name="Ekimus"/>
        <npc id="29151" type="Monster" level="82" name="Feral Hound of the Netherworld"/>"#;
    const ITEMS: &str = r#"<item id="8871" type="EtcItem" name="Strategy Guide"/>
        <item id="13797" type="EtcItem" name="Soul Stone Fragment"><set name="is_stackable" val="true"/></item>"#;

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        t.npcs_of(npc_id)
            .into_iter()
            .filter(|n| !t.world.is_dead(*n))
            .collect()
    }

    #[test]
    fn the_channel_reaches_ekimus_cuts_his_tumors_and_kills_him() {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfInfinityManager);
        registry.add(HeartInfinityAttack);
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &[], |world, _| {
            world.set_script_settings(l2_world::ScriptSettings {
                cycle_database_writes: true,
                gracia_seeds: l2_world::script::GraciaSeedsSettings {
                    heart_attack_min_players: 2,
                    ..Default::default()
                },
                ..Default::default()
            });
            world.set_global_variables(vec![("SoI_stage".to_owned(), Some("2".to_owned()))]);
        });
        for id in 1..=4 {
            let items = if id == 1 {
                vec![item(1000, STRATEGY_GUIDE, 1)]
            } else {
                vec![]
            };
            t.enter_with(player_data(id, 80, items));
        }
        command_channel(&mut t, &[&[1, 2], &[3, 4]]);
        let gaze = ScriptNpc {
            object_id: t.spawn(ABYSSGAZE, 20),
            id: ABYSSGAZE,
        };
        t.world.run_as_script(QN, |ctx| {
            HeartInfinityAttack.on_talk(ctx, gaze, Oid(1));
        });
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_id_of(Oid(4)), instance);
        assert_eq!(t.npcs_of(32536).len(), 1);

        // The leader takes his party to Ekimus: the movie after 20 s, the fight 62.5 s later.
        t.world.run_as_script(QN, |ctx| {
            ctx.notify_event("warpechmus", None, Some(Oid(1)));
        });
        t.wait_ms(20_100);
        assert!(t.world.is_playing_movie(Oid(1)));
        t.wait_ms(62_600);
        assert!(t.npcs_of(32536).is_empty());
        let tumors = alive(&t, ALIVETUMOR);
        assert_eq!(tumors.len(), 6);
        assert_eq!(t.world.creature_hp(tumors[0]).map(|h| h.0), Some(500.0));
        let ekimus = alive(&t, EKIMUS);
        assert_eq!(ekimus.len(), 1);
        let hounds = alive(&t, HOUND);
        assert_eq!(hounds.len(), 2);

        // All tumors down: the hounds are out of Ekimus' control.
        for tumor in tumors {
            t.kill(1, tumor);
        }
        assert!(t.world.is_invul(hounds[0]));
        assert_eq!(t.npcs_of(DEADTUMOR).len(), 6);

        // Ekimus dies: the seed counts it, the instance ends.
        t.kill(1, ekimus[0]);
        assert_eq!(t.world.global_variables().get_int("SoI_EkimusCount", 0), 1);
        assert!(t.npcs_of(DEADTUMOR).is_empty());
        // The tumors do not come back.
        t.wait_ms(181_000);
        assert!(alive(&t, ALIVETUMOR).is_empty());
    }
}
